//! Test-only AI block that waits for a response, for tests that need an AI block in the blocklist.
use std::rc::Rc;

use warpui::{AppContext, ViewContext, ViewHandle};

use crate::ai::{
    agent::{conversation::AIConversationId, AIAgentInput, ServerOutputId, UserQueryMode},
    blocklist::{
        model::{AIBlockModel, AIBlockOutputStatus, AIRequestType, OutputStatusUpdateCallback},
        AIBlock, ClientIdentifiers,
    },
    llms::LLMId,
};

use super::{
    AIBlockMetadata, RichContentInsertionPosition, RichContentMetadata, RichContentType,
    TerminalView,
};

struct PendingAIBlockModel {
    conversation_id: AIConversationId,
    input: Vec<AIAgentInput>,
    model_id: LLMId,
}

impl AIBlockModel for PendingAIBlockModel {
    type View = AIBlock;

    fn status(&self, _app: &AppContext) -> AIBlockOutputStatus {
        AIBlockOutputStatus::Pending
    }

    fn server_output_id(&self, _app: &AppContext) -> Option<ServerOutputId> {
        None
    }

    fn model_id(&self, _app: &AppContext) -> Option<LLMId> {
        None
    }

    fn base_model<'a>(&'a self, _app: &'a AppContext) -> Option<&'a LLMId> {
        Some(&self.model_id)
    }

    fn inputs_to_render<'a>(&'a self, _app: &'a AppContext) -> &'a [AIAgentInput] {
        &self.input
    }

    fn conversation_id(&self, _app: &AppContext) -> Option<AIConversationId> {
        Some(self.conversation_id)
    }

    fn on_updated_output(
        &self,
        _callback: OutputStatusUpdateCallback<AIBlock>,
        _ctx: &mut ViewContext<AIBlock>,
    ) {
    }

    fn request_type(&self, _app: &AppContext) -> AIRequestType {
        AIRequestType::Active
    }
}

impl TerminalView {
    /// Appends an AI block for `query` in `conversation_id` that waits for a response.
    pub(crate) fn insert_pending_ai_block_for_test(
        &mut self,
        conversation_id: AIConversationId,
        query: &str,
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<AIBlock> {
        let ai_block_model = Rc::new(PendingAIBlockModel {
            conversation_id,
            input: vec![AIAgentInput::UserQuery {
                query: query.to_owned(),
                context: vec![].into(),
                static_query_type: None,
                referenced_attachments: Default::default(),
                user_query_mode: UserQueryMode::default(),
                running_command: None,
                intended_agent: None,
            }],
            model_id: LLMId::from("fake-llm"),
        });
        let ai_block = ctx.add_typed_action_view(|ctx| {
            AIBlock::new(
                ai_block_model,
                self.model.clone(),
                ClientIdentifiers {
                    client_exchange_id: Default::default(),
                    conversation_id,
                    response_stream_id: None,
                },
                self.ai_controller.clone(),
                self.get_relevant_files_controller.clone(),
                None,
                None,
                self.ai_action_model.clone(),
                self.ai_context_model.clone(),
                self.find_model.clone(),
                self.active_session.clone(),
                self.ambient_agent_view_model.clone(),
                &self.cli_subagent_controller,
                &self.model_events_handle,
                self.agent_view_controller.clone(),
                self.view_handle.clone(),
                self.id(),
                ctx,
            )
        });

        self.insert_rich_content(
            Some(RichContentType::AIBlock),
            ai_block.clone(),
            Some(RichContentMetadata::AIBlock(AIBlockMetadata {
                exchange_id: Default::default(),
                conversation_id,
                ai_block_handle: ai_block.clone(),
            })),
            RichContentInsertionPosition::Append {
                insert_below_long_running_block: false,
            },
            ctx,
        );
        ai_block
    }
}
