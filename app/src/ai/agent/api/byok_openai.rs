use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ::ai::api_keys::OpenAICompatibleProviderConfig;
use anyhow::anyhow;
use async_stream::stream;
use futures_util::StreamExt;
use prost_types::{FieldMask, Timestamp};
use serde::{Deserialize, Serialize};
use url::Url;
use warp_multi_agent_api as api;

use crate::{
    ai::agent::{AIAgentContext, AIAgentInput, AnyFileContent, RunningCommand},
    server::server_api::{AIApiError, ServerApi},
};

use super::{ConvertToAPITypeError, RequestParams, ResponseStream};

const MAX_CONTEXT_CHARS: usize = 12_000;
const MAX_BLOCK_OUTPUT_CHARS: usize = 4_000;
const RESUME_CONVERSATION_PROMPT: &str = "Please continue the conversation from the latest point. If the previous response was interrupted or failed, resume without repeating completed content.";

pub async fn generate_chat_output(
    server_api: Arc<ServerApi>,
    params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    let request = match OpenAIChatRequest::from_params(&params) {
        Ok(request) => request,
        Err(err) => return Ok(error_stream(err)),
    };

    let stream_context = StreamContext::from_params(&params, &request.model);
    let mut request_builder = server_api
        .http_client()
        .post(request.url.clone())
        .json(&request.body);

    if let Some(api_key) = request.api_key.as_ref() {
        request_builder = request_builder.bearer_auth(api_key);
    }

    let request_builder = request_builder.prevent_sleep("OpenAI BYOK chat request in-progress");
    let mut event_source = request_builder.eventsource();

    let output_stream = stream! {
        yield Ok(stream_context.init_event());

        if let Some(create_task_event) = stream_context.create_task_event() {
            yield Ok(create_task_event);
        }

        yield Ok(stream_context.model_used_event());
        yield Ok(stream_context.initial_output_event());

        while let Some(event) = event_source.next().await {
            match event {
                Ok(reqwest_eventsource::Event::Open) => {}
                Ok(reqwest_eventsource::Event::Message(message)) => {
                    let data = message.data.trim();
                    if data == "[DONE]" {
                        yield Ok(stream_context.finished_event(None));
                        break;
                    }

                    match parse_openai_stream_data(data) {
                        Ok(OpenAIStreamItem::Delta(delta)) => {
                            if !delta.is_empty() {
                                yield Ok(stream_context.append_output_event(delta));
                            }
                        }
                        Ok(OpenAIStreamItem::Finished(reason)) => {
                            yield Ok(stream_context.finished_event(reason.as_deref()));
                            break;
                        }
                        Err(err) => {
                            yield Err(Arc::new(err));
                            break;
                        }
                    }
                }
                Err(err) => {
                    yield Err(Arc::new(openai_stream_error(err).await));
                    break;
                }
            }
        }
    }
    .take_until(cancellation_rx);

    cfg_if::cfg_if! {
        if #[cfg(target_family = "wasm")] {
            Ok(output_stream.boxed_local())
        } else {
            Ok(output_stream.boxed())
        }
    }
}

fn error_stream(error: AIApiError) -> ResponseStream {
    let output_stream = futures::stream::once(async move { Err(Arc::new(error)) });
    cfg_if::cfg_if! {
        if #[cfg(target_family = "wasm")] {
            output_stream.boxed_local()
        } else {
            output_stream.boxed()
        }
    }
}

#[derive(Debug)]
struct OpenAIChatRequest {
    url: String,
    api_key: Option<String>,
    model: String,
    body: OpenAIChatCompletionRequest,
}

impl OpenAIChatRequest {
    fn from_params(params: &RequestParams) -> Result<Self, AIApiError> {
        let config = &params.openai_compatible_provider;
        let url = chat_completions_url_for_base_url(config.effective_base_url())?;
        let api_key = config.effective_api_key().map(ToOwned::to_owned);

        // OpenAI's hosted endpoint requires a credential. Custom compatible
        // endpoints can be local or proxy-backed and may not require one.
        if api_key.is_none() && is_default_openai_chat_completions_url(&url) {
            return Err(AIApiError::Other(anyhow!(
                "OpenAI API key is required for BYOK chat. Add it in Settings > BYOK."
            )));
        }

        let user_prompt = user_prompt_from_inputs(&params.input).ok_or_else(|| {
            AIApiError::Other(anyhow!(
                "BYOK OpenAI chat currently supports user text queries only."
            ))
        })?;

        let model = model_for_provider_config(config);
        let mut messages = conversation_messages_from_tasks(&params.tasks);
        messages.push(OpenAIMessage {
            role: OpenAIRole::User,
            content: user_prompt,
        });

        Ok(Self {
            url,
            api_key,
            model: model.clone(),
            body: OpenAIChatCompletionRequest {
                model,
                messages,
                stream: true,
            },
        })
    }
}

#[derive(Debug, Serialize)]
struct OpenAIChatCompletionRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct OpenAIMessage {
    role: OpenAIRole,
    content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum OpenAIRole {
    System,
    User,
    Assistant,
}

fn conversation_messages_from_tasks(tasks: &[api::Task]) -> Vec<OpenAIMessage> {
    let mut messages = vec![OpenAIMessage {
        role: OpenAIRole::System,
        content: "You are Warp's local BYOK terminal chat assistant. Use the provided terminal context when it is relevant. Do not claim that you ran commands or changed files.".to_string(),
    }];

    for task in tasks {
        for message in &task.messages {
            match message.message.as_ref() {
                Some(api::message::Message::UserQuery(query)) if !query.query.is_empty() => {
                    messages.push(OpenAIMessage {
                        role: OpenAIRole::User,
                        content: truncate_chars(&query.query, MAX_CONTEXT_CHARS),
                    });
                }
                Some(api::message::Message::AgentOutput(output)) if !output.text.is_empty() => {
                    messages.push(OpenAIMessage {
                        role: OpenAIRole::Assistant,
                        content: truncate_chars(&output.text, MAX_CONTEXT_CHARS),
                    });
                }
                _ => {}
            }
        }
    }

    messages
}

fn user_prompt_from_inputs(inputs: &[AIAgentInput]) -> Option<String> {
    inputs.iter().rev().find_map(|input| match input {
        AIAgentInput::UserQuery {
            query,
            context,
            running_command,
            ..
        } => Some(render_user_prompt(query, context, running_command.as_ref())),
        AIAgentInput::AutoCodeDiffQuery { query, context } => {
            Some(render_user_prompt(query, context, None))
        }
        AIAgentInput::ResumeConversation { context } => Some(render_user_prompt(
            RESUME_CONVERSATION_PROMPT,
            context,
            None,
        )),
        _ => None,
    })
}

fn render_user_prompt(
    query: &str,
    context: &[AIAgentContext],
    running_command: Option<&RunningCommand>,
) -> String {
    let mut prompt = String::new();
    let rendered_context = render_context(context, running_command);

    if !rendered_context.is_empty() {
        prompt.push_str("Terminal context:\n");
        prompt.push_str(&truncate_chars(&rendered_context, MAX_CONTEXT_CHARS));
        prompt.push_str("\n\n");
    }

    prompt.push_str("User request:\n");
    prompt.push_str(query);
    prompt
}

fn render_context(context: &[AIAgentContext], running_command: Option<&RunningCommand>) -> String {
    let mut lines = Vec::new();

    for item in context {
        match item {
            AIAgentContext::Directory { pwd, home_dir, .. } => {
                if let Some(pwd) = pwd {
                    lines.push(format!("Working directory: {pwd}"));
                }
                if let Some(home_dir) = home_dir {
                    lines.push(format!("Home directory: {home_dir}"));
                }
            }
            AIAgentContext::CurrentTime { current_time } => {
                lines.push(format!("Current time: {current_time}"));
            }
            AIAgentContext::ExecutionEnvironment(env) => {
                if let Some(env) = env.to_json_string() {
                    lines.push(format!("Execution environment: {env}"));
                }
            }
            AIAgentContext::SelectedText(text) => {
                lines.push(format!(
                    "Selected text:\n{}",
                    truncate_chars(text, MAX_BLOCK_OUTPUT_CHARS)
                ));
            }
            AIAgentContext::Block(block) => {
                lines.push(format!(
                    "Completed command:\n$ {}\nExit code: {}\nOutput:\n{}",
                    block.command,
                    block.exit_code,
                    truncate_chars(&block.output, MAX_BLOCK_OUTPUT_CHARS)
                ));
            }
            AIAgentContext::Git { head, branch } => {
                let branch = branch.as_deref().unwrap_or("unknown");
                lines.push(format!("Git branch: {branch}\nGit head: {head}"));
            }
            AIAgentContext::Codebase { path, name } => {
                lines.push(format!("Codebase: {name} at {path}"));
            }
            AIAgentContext::ProjectRules {
                root_path,
                active_rules,
                ..
            } => {
                lines.push(format!(
                    "Project rules root: {root_path}\nActive rule files: {}",
                    active_rules.len()
                ));
            }
            AIAgentContext::File(file) => {
                let content = match &file.content {
                    AnyFileContent::StringContent(content) => {
                        truncate_chars(content, MAX_BLOCK_OUTPUT_CHARS)
                    }
                    AnyFileContent::BinaryContent(content) => {
                        format!("[binary file, {} bytes]", content.len())
                    }
                };
                lines.push(format!("Attached file: {}\n{}", file.file_name, content));
            }
            AIAgentContext::Image(image) => {
                lines.push(format!("Attached image: {}", image.file_name));
            }
            AIAgentContext::Skills { skills } => {
                lines.push(format!("Available skills: {}", skills.len()));
            }
        }
    }

    if let Some(command) = running_command {
        lines.push(format!(
            "Running command:\n$ {}\nCurrent output:\n{}",
            command.command,
            truncate_chars(&command.grid_contents, MAX_BLOCK_OUTPUT_CHARS)
        ));
    }

    lines.join("\n\n")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let chars_to_take = max_chars.saturating_sub(15);
    let truncated: String = text.chars().take(chars_to_take).collect();
    format!("{truncated}\n...[truncated]")
}

fn model_for_provider_config(config: &OpenAICompatibleProviderConfig) -> String {
    config.effective_model().to_string()
}

fn is_default_openai_chat_completions_url(url: &str) -> bool {
    match chat_completions_url_for_base_url(::ai::api_keys::OPENAI_COMPATIBLE_DEFAULT_BASE_URL) {
        Ok(default_url) => url == default_url,
        Err(_) => false,
    }
}

fn chat_completions_url_for_base_url(base_url: &str) -> Result<String, AIApiError> {
    let mut url = Url::parse(base_url.trim()).map_err(|err| {
        AIApiError::Other(anyhow!(err).context("OpenAI-compatible BYOK base URL is invalid"))
    })?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(AIApiError::Other(anyhow!(
            "OpenAI-compatible BYOK base URL must use http or https."
        )));
    }

    let trimmed_path = url.path().trim_end_matches('/');
    let chat_completions_path = if trimmed_path.ends_with("/chat/completions") {
        trimmed_path.to_string()
    } else if trimmed_path.is_empty() {
        "/chat/completions".to_string()
    } else {
        format!("{trimmed_path}/chat/completions")
    };

    url.set_path(&chat_completions_path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorEnvelope {
    error: Option<OpenAIError>,
}

#[derive(Debug, Deserialize)]
struct OpenAIError {
    message: String,
}

enum OpenAIStreamItem {
    Delta(String),
    Finished(Option<String>),
}

fn parse_openai_stream_data(data: &str) -> Result<OpenAIStreamItem, AIApiError> {
    let chunk = serde_json::from_str::<OpenAIStreamChunk>(data).or_else(|chunk_error| {
        match serde_json::from_str::<OpenAIErrorEnvelope>(data) {
            Ok(OpenAIErrorEnvelope {
                error: Some(error), ..
            }) => Err(anyhow!(error.message)),
            _ => Err(anyhow!(chunk_error).context("Failed to parse OpenAI stream event")),
        }
    })?;

    let mut content = String::new();
    let mut finish_reason = None;
    for choice in chunk.choices {
        if let Some(delta) = choice.delta.content {
            content.push_str(&delta);
        }
        if choice.finish_reason.is_some() {
            finish_reason = choice.finish_reason;
        }
    }

    if !content.is_empty() {
        Ok(OpenAIStreamItem::Delta(content))
    } else if finish_reason.is_some() {
        Ok(OpenAIStreamItem::Finished(finish_reason))
    } else {
        Ok(OpenAIStreamItem::Delta(String::new()))
    }
}

async fn openai_stream_error(err: reqwest_eventsource::Error) -> AIApiError {
    match err {
        reqwest_eventsource::Error::InvalidStatusCode(status, response) => AIApiError::ErrorStatus(
            status,
            response
                .text()
                .await
                .unwrap_or_else(|e| format!("(no response body: {e:#})")),
        ),
        reqwest_eventsource::Error::Transport(err) => AIApiError::from(err),
        err => AIApiError::Other(anyhow!(err).context("OpenAI BYOK stream failed")),
    }
}

#[derive(Debug, Clone)]
struct StreamContext {
    conversation_id: String,
    request_id: String,
    task_id: String,
    message_id: String,
    model: String,
    should_create_task: bool,
}

impl StreamContext {
    fn from_params(params: &RequestParams, model: &str) -> Self {
        let conversation_id = params
            .conversation_token
            .as_ref()
            .map(|token| token.as_str().to_string())
            .unwrap_or_else(new_id);
        let task_id = root_task_id(&params.tasks).unwrap_or_else(new_id);

        Self {
            conversation_id: conversation_id.clone(),
            request_id: new_id(),
            task_id,
            message_id: new_id(),
            model: model.to_string(),
            should_create_task: params.tasks.is_empty(),
        }
    }

    fn init_event(&self) -> api::ResponseEvent {
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Init(
                api::response_event::StreamInit {
                    conversation_id: self.conversation_id.clone(),
                    request_id: self.request_id.clone(),
                    run_id: self.conversation_id.clone(),
                },
            )),
        }
    }

    fn create_task_event(&self) -> Option<api::ResponseEvent> {
        self.should_create_task.then(|| {
            client_actions_event(vec![api::ClientAction {
                action: Some(api::client_action::Action::CreateTask(
                    api::client_action::CreateTask {
                        task: Some(api::Task {
                            id: self.task_id.clone(),
                            description: "BYOK chat".to_string(),
                            dependencies: None,
                            messages: vec![],
                            summary: String::new(),
                            server_data: String::new(),
                        }),
                    },
                )),
            }])
        })
    }

    fn model_used_event(&self) -> api::ResponseEvent {
        let message = self.message(api::message::Message::ModelUsed(api::message::ModelUsed {
            model_id: self.model.clone(),
            model_display_name: self.model.clone(),
            is_fallback: false,
        }));
        client_actions_event(vec![add_messages_action(&self.task_id, vec![message])])
    }

    fn initial_output_event(&self) -> api::ResponseEvent {
        let message = self.message(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: String::new(),
            },
        ));
        client_actions_event(vec![add_messages_action(&self.task_id, vec![message])])
    }

    fn append_output_event(&self, delta: String) -> api::ResponseEvent {
        let message = self.message(api::message::Message::AgentOutput(
            api::message::AgentOutput { text: delta },
        ));
        client_actions_event(vec![api::ClientAction {
            action: Some(api::client_action::Action::AppendToMessageContent(
                api::client_action::AppendToMessageContent {
                    task_id: self.task_id.clone(),
                    message: Some(message),
                    mask: Some(FieldMask {
                        paths: vec!["agent_output.text".to_string()],
                    }),
                },
            )),
        }])
    }

    fn finished_event(&self, finish_reason: Option<&str>) -> api::ResponseEvent {
        let reason = match finish_reason {
            Some("length") => api::response_event::stream_finished::Reason::MaxTokenLimit(
                api::response_event::stream_finished::ReachedMaxTokenLimit {},
            ),
            _ => api::response_event::stream_finished::Reason::Done(
                api::response_event::stream_finished::Done {},
            ),
        };

        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Finished(
                api::response_event::StreamFinished {
                    reason: Some(reason),
                    conversation_usage_metadata: Some(
                        api::response_event::stream_finished::ConversationUsageMetadata {
                            context_window_usage: 0.0,
                            summarized: false,
                            credits_spent: 0.0,
                            #[allow(deprecated)]
                            token_usage: vec![],
                            tool_usage_metadata: None,
                            warp_token_usage: Default::default(),
                            byok_token_usage: Default::default(),
                        },
                    ),
                    token_usage: vec![],
                    should_refresh_model_config: false,
                    request_cost: None,
                },
            )),
        }
    }

    fn message(&self, message: api::message::Message) -> api::Message {
        api::Message {
            id: self.message_id.clone(),
            task_id: self.task_id.clone(),
            request_id: self.request_id.clone(),
            timestamp: current_timestamp(),
            server_message_data: String::new(),
            citations: vec![],
            message: Some(message),
        }
    }
}

fn client_actions_event(actions: Vec<api::ClientAction>) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions { actions },
        )),
    }
}

fn add_messages_action(task_id: &str, messages: Vec<api::Message>) -> api::ClientAction {
    api::ClientAction {
        action: Some(api::client_action::Action::AddMessagesToTask(
            api::client_action::AddMessagesToTask {
                task_id: task_id.to_string(),
                messages,
            },
        )),
    }
}

fn root_task_id(tasks: &[api::Task]) -> Option<String> {
    tasks
        .iter()
        .find(|task| {
            task.dependencies
                .as_ref()
                .is_none_or(|dependencies| dependencies.parent_task_id.is_empty())
        })
        .map(|task| task.id.clone())
}

fn current_timestamp() -> Option<Timestamp> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    let seconds = i64::try_from(duration.as_secs()).ok()?;
    Some(Timestamp {
        seconds,
        nanos: duration.subsec_nanos() as i32,
    })
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use crate::ai::{
        agent::{AIAgentContext, AIAgentInput, UserQueryMode},
        block_context::BlockContext,
        blocklist::SessionContext,
        llms::LLMId,
    };
    use warp_core::command::ExitCode;

    use super::*;

    fn request_params_with_openai_provider(
        openai_compatible_provider: OpenAICompatibleProviderConfig,
    ) -> RequestParams {
        let input = AIAgentInput::UserQuery {
            query: "hi".to_string(),
            context: Arc::from(Vec::<AIAgentContext>::new()),
            static_query_type: None,
            referenced_attachments: HashMap::new(),
            user_query_mode: UserQueryMode::Normal,
            running_command: None,
            intended_agent: None,
        };

        request_params_with_openai_provider_and_input(openai_compatible_provider, input)
    }

    fn request_params_with_openai_provider_and_input(
        openai_compatible_provider: OpenAICompatibleProviderConfig,
        input: AIAgentInput,
    ) -> RequestParams {
        let model = LLMId::from("test-model");

        RequestParams {
            input: vec![input],
            conversation_token: None,
            forked_from_conversation_token: None,
            ambient_agent_task_id: None,
            tasks: vec![],
            existing_suggestions: None,
            metadata: None,
            session_context: SessionContext::new_for_test(),
            model: model.clone(),
            coding_model: model.clone(),
            cli_agent_model: model.clone(),
            computer_use_model: model,
            is_memory_enabled: false,
            warp_drive_context_enabled: false,
            mcp_context: None,
            planning_enabled: true,
            should_redact_secrets: false,
            api_keys: None,
            openai_compatible_provider,
            allow_use_of_warp_credits_with_byok: false,
            autonomy_level: api::AutonomyLevel::Supervised,
            isolation_level: api::IsolationLevel::None,
            web_search_enabled: false,
            computer_use_enabled: false,
            ask_user_question_enabled: false,
            research_agent_enabled: false,
            orchestration_enabled: false,
            supported_tools_override: None,
            parent_agent_id: None,
            agent_name: None,
        }
    }

    #[test]
    fn uses_provider_model() {
        let config = OpenAICompatibleProviderConfig {
            model: "custom-model".to_string(),
            ..Default::default()
        };

        assert_eq!(model_for_provider_config(&config), "custom-model");
    }

    #[test]
    fn maps_empty_provider_model_to_default() {
        let config = OpenAICompatibleProviderConfig {
            model: " ".to_string(),
            ..Default::default()
        };

        assert_eq!(
            model_for_provider_config(&config),
            ::ai::api_keys::OPENAI_COMPATIBLE_DEFAULT_MODEL
        );
    }

    #[test]
    fn custom_openai_compatible_endpoint_allows_missing_api_key() {
        let config = OpenAICompatibleProviderConfig {
            base_url: "http://localhost:11434/v1".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
        };
        let params = request_params_with_openai_provider(config);

        let request = OpenAIChatRequest::from_params(&params)
            .expect("custom compatible endpoint should not require an API key");

        assert_eq!(request.api_key, None);
        assert_eq!(request.url, "http://localhost:11434/v1/chat/completions");
    }

    #[test]
    fn default_openai_endpoint_requires_api_key() {
        let params = request_params_with_openai_provider(OpenAICompatibleProviderConfig::default());

        let err =
            OpenAIChatRequest::from_params(&params).expect_err("default OpenAI endpoint needs key");

        assert!(err.to_string().contains("API key is required"));
    }

    #[test]
    fn custom_openai_compatible_endpoint_preserves_api_key() {
        let config = OpenAICompatibleProviderConfig {
            base_url: "https://proxy.example.test/openai/v1".to_string(),
            model: "proxy-model".to_string(),
            api_key: Some("proxy-key".to_string()),
        };
        let params = request_params_with_openai_provider(config);

        let request = OpenAIChatRequest::from_params(&params)
            .expect("custom compatible endpoint with API key should be valid");

        assert_eq!(request.api_key, Some("proxy-key".to_string()));
        assert_eq!(request.model, "proxy-model");
    }

    #[test]
    fn resume_conversation_input_builds_chat_request() {
        let config = OpenAICompatibleProviderConfig {
            base_url: "http://localhost:11434/v1".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
        };
        let params = request_params_with_openai_provider_and_input(
            config,
            AIAgentInput::ResumeConversation {
                context: Arc::from(Vec::<AIAgentContext>::new()),
            },
        );

        let request = OpenAIChatRequest::from_params(&params)
            .expect("resume conversation should be supported for BYOK OpenAI chat");

        let last_message = request
            .body
            .messages
            .last()
            .expect("request should include a resume prompt");
        assert_eq!(last_message.role, OpenAIRole::User);
        assert!(last_message.content.contains("Please continue"));
    }

    #[test]
    fn builds_chat_completions_url_from_base_url() {
        let url = chat_completions_url_for_base_url("https://example.test/v1/")
            .expect("base URL should be valid");

        assert_eq!(url, "https://example.test/v1/chat/completions");
    }

    #[test]
    fn rejects_non_http_chat_completions_url() {
        let err = chat_completions_url_for_base_url("file:///tmp/openai")
            .expect_err("file URL should be rejected");

        assert!(err.to_string().contains("http or https"));
    }

    #[test]
    fn renders_terminal_context_for_user_query() {
        let context = vec![AIAgentContext::Block(Box::new(BlockContext {
            id: Default::default(),
            index: Default::default(),
            command: "cargo test".to_string(),
            output: "test result: ok".to_string(),
            exit_code: ExitCode::from(0),
            is_auto_attached: false,
            started_ts: None,
            finished_ts: None,
            pwd: Some("/tmp/project".to_string()),
            shell: Some("zsh".to_string()),
            username: Some("user".to_string()),
            hostname: Some("host".to_string()),
            git_branch: Some("main".to_string()),
            os: Some("macOS".to_string()),
            session_id: Some(7),
        }))];
        let input = AIAgentInput::UserQuery {
            query: "Why did this fail?".to_string(),
            context: Arc::from(context),
            static_query_type: None,
            referenced_attachments: HashMap::new(),
            user_query_mode: UserQueryMode::Normal,
            running_command: None,
            intended_agent: None,
        };

        let prompt = user_prompt_from_inputs(&[input]).expect("user query should render");

        assert!(prompt.contains("Terminal context:"));
        assert!(prompt.contains("$ cargo test"));
        assert!(prompt.contains("test result: ok"));
        assert!(prompt.contains("User request:\nWhy did this fail?"));
    }

    #[test]
    fn renders_resume_conversation_prompt() {
        let input = AIAgentInput::ResumeConversation {
            context: Arc::from(Vec::<AIAgentContext>::new()),
        };

        let prompt = user_prompt_from_inputs(&[input]).expect("resume input should render");

        assert!(prompt.contains("User request:\nPlease continue"));
    }

    #[test]
    fn parses_openai_stream_delta() {
        let item = parse_openai_stream_data(
            r#"{"choices":[{"delta":{"content":"hello"},"finish_reason":null}]}"#,
        )
        .expect("valid stream item");

        match item {
            OpenAIStreamItem::Delta(delta) => assert_eq!(delta, "hello"),
            OpenAIStreamItem::Finished(_) => panic!("expected delta"),
        }
    }

    #[test]
    fn append_output_mask_appends_agent_output_text() {
        let stream_context = StreamContext {
            conversation_id: "conversation-id".to_string(),
            request_id: "request-id".to_string(),
            task_id: "task-id".to_string(),
            message_id: "message-id".to_string(),
            model: "gpt-4o".to_string(),
            should_create_task: false,
        };
        let existing = stream_context.message(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: "hello".to_string(),
            },
        ));
        let delta = stream_context.message(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: " world".to_string(),
            },
        ));

        let updated = field_mask::FieldMaskOperation::append(
            &api::MESSAGE_DESCRIPTOR,
            &existing,
            &delta,
            FieldMask {
                paths: vec!["agent_output.text".to_string()],
            },
        )
        .apply()
        .expect("agent output append mask should be valid");

        match updated
            .message
            .expect("updated message should have content")
        {
            api::message::Message::AgentOutput(output) => assert_eq!(output.text, "hello world"),
            other => panic!("expected agent output, got {other:?}"),
        }
    }
}
