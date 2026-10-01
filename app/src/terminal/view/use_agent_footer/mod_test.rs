use session_sharing_protocol::sharer::SessionSourceType;
use warp_core::settings::Setting as _;
use warpui::{App, SingletonEntity, ViewContext};

use crate::{
    ai::{
        agent::{conversation::AIConversationId, task::TaskId},
        blocklist::{agent_view::AgentViewEntryOrigin, block::cli_controller::UserTakeOverReason},
    },
    features::FeatureFlag,
    settings::AISettings,
    terminal::cli_agent_sessions::{
        CLIAgentInputState, CLIAgentSession, CLIAgentSessionContext, CLIAgentSessionStatus,
        CLIAgentSessionsModel,
    },
    terminal::model::ansi::{BootstrappedValue, Handler as _, InitShellValue},
    terminal::CLIAgent,
    test_util::{add_window_with_terminal, terminal::initialize_app_for_terminal_view},
};

use super::*;

fn simulate_user_started_long_running_command(view: &mut TerminalView) {
    {
        let mut model = view.model.lock();
        model.init_shell(InitShellValue {
            session_id: 0.into(),
            shell: "zsh".to_owned(),
            ..Default::default()
        });
        model.bootstrapped(BootstrappedValue {
            shell: "zsh".to_owned(),
            ..Default::default()
        });
        model.simulate_long_running_block("ssh localhost", "Password:");
    }
}

fn transition_to_user_handoff_state(
    view: &mut TerminalView,
    reason: UserTakeOverReason,
    ctx: &mut ViewContext<TerminalView>,
) -> AIConversationId {
    let conversation_id = view.agent_view_controller().update(ctx, |controller, ctx| {
        controller
            .try_enter_inline_agent_view(None, AgentViewEntryOrigin::LongRunningCommand, ctx)
            .expect("inline agent view should create a conversation")
    });
    view.model
        .lock()
        .block_list_mut()
        .active_block_mut()
        .set_is_agent_tagged_in(true);

    let task_id = TaskId::new("test-task".to_owned());
    view.model
        .lock()
        .block_list_mut()
        .active_block_mut()
        .set_agent_interaction_mode_for_agent_monitored_command(&task_id, conversation_id)
        .expect("tagged-in command should transition to agent-monitored");

    view.cli_subagent_controller.update(ctx, |controller, ctx| {
        controller.switch_control_to_user(reason, ctx);
    });

    conversation_id
}

#[test]
fn use_agent_footer_renders_for_manual_handoff_even_when_user_command_footer_setting_disabled() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        FeatureFlag::AgentView.set_enabled(true);
        AISettings::handle(&app).update(&mut app, |settings, ctx| {
            let _ = settings
                .should_render_use_agent_footer_for_user_commands
                .set_value(false, ctx);
        });

        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            simulate_user_started_long_running_command(view);

            view.maybe_show_use_agent_footer_in_blocklist(ctx);
            {
                let model = view.model.lock();
                assert!(!view.should_render_use_agent_footer(&model, ctx));
                let active_block_index = model.block_list().active_block_index();
                assert!(model
                    .block_list()
                    .last_non_hidden_rich_content_block_after_block(Some(active_block_index))
                    .is_none());
            }

            transition_to_user_handoff_state(view, UserTakeOverReason::Manual, ctx);

            view.maybe_show_use_agent_footer_in_blocklist(ctx);
            let model = view.model.lock();
            assert!(view.should_render_use_agent_footer(&model, ctx));
            let active_block_index = model.block_list().active_block_index();
            let rendered_footer_view_id = model
                .block_list()
                .last_non_hidden_rich_content_block_after_block(Some(active_block_index))
                .map(|(_, item)| item.view_id);
            assert_eq!(rendered_footer_view_id, Some(view.use_agent_footer.id()));
        });
    })
}

#[test]
fn use_agent_footer_renders_for_manual_handoff_when_unfinished_ai_block_remains() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        FeatureFlag::AgentView.set_enabled(true);

        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            simulate_user_started_long_running_command(view);

            let conversation_id = view.agent_view_controller().update(ctx, |controller, ctx| {
                controller
                    .try_enter_inline_agent_view(
                        None,
                        AgentViewEntryOrigin::LongRunningCommand,
                        ctx,
                    )
                    .expect("inline agent view should create a conversation")
            });
            view.model
                .lock()
                .block_list_mut()
                .active_block_mut()
                .set_is_agent_tagged_in(true);
            let task_id = TaskId::new("test-task".to_owned());
            view.model
                .lock()
                .block_list_mut()
                .active_block_mut()
                .set_agent_interaction_mode_for_agent_monitored_command(&task_id, conversation_id)
                .expect("tagged-in command should transition to agent-monitored");

            view.insert_pending_ai_block_for_test(
                conversation_id,
                "help with this running command",
                ctx,
            );
            assert!(view.active_ai_block(ctx).is_some());

            view.cli_subagent_controller.update(ctx, |controller, ctx| {
                controller.switch_control_to_user(UserTakeOverReason::Manual, ctx);
            });
        });

        terminal.read(&app, |view, ctx| {
            let model = view.model.lock();
            assert!(view.should_render_use_agent_footer(&model, ctx));
            let active_block_index = model.block_list().active_block_index();
            let rendered_footer_view_id = model
                .block_list()
                .last_non_hidden_rich_content_block_after_block(Some(active_block_index))
                .map(|(_, item)| item.view_id);
            assert_eq!(rendered_footer_view_id, Some(view.use_agent_footer.id()));
        });
    })
}

/// During the setup phase of a cloud agent (ambient) shared session — LRCs
/// running before any CLI agent has started — the use-agent footer must stay
/// hidden.
#[test]
fn use_agent_footer_hidden_during_cloud_agent_setup_lrc() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);

        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            simulate_user_started_long_running_command(view);

            // Cloud agent setup phase: ambient source type set, LRC running,
            // NO CLIAgentSession registered yet.
            view.model
                .lock()
                .set_shared_session_source_type(SessionSourceType::AmbientAgent { task_id: None });
            assert!(view.model.lock().is_shared_ambient_agent_session());
            assert!(
                CLIAgentSessionsModel::as_ref(ctx)
                    .session(view.id())
                    .is_none(),
                "precondition: no CLI agent session yet",
            );

            view.maybe_show_use_agent_footer_in_blocklist(ctx);

            let model = view.model.lock();
            assert!(
                !view.should_render_use_agent_footer(&model, ctx),
                "footer should be hidden during cloud agent setup LRCs",
            );
            let active_block_index = model.block_list().active_block_index();
            assert!(
                model
                    .block_list()
                    .last_non_hidden_rich_content_block_after_block(Some(active_block_index))
                    .is_none(),
                "footer rich content should not be in the blocklist during cloud setup",
            );
        });
    })
}

/// When viewing a shared cloud-agent (ambient agent) session whose sharer is
/// running a CLI agent, the CLI agent footer should still render.
#[test]
fn cli_agent_footer_renders_for_viewer_of_shared_cloud_agent_session() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);

        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            simulate_user_started_long_running_command(view);

            // Mark the model as a shared ambient (cloud) agent session, mirroring
            // what the viewer's terminal manager does on `JoinedSuccessfully`.
            view.model
                .lock()
                .set_shared_session_source_type(SessionSourceType::AmbientAgent { task_id: None });
            assert!(view.model.lock().is_shared_ambient_agent_session());

            // Inject a CLI agent session as `apply_cli_agent_state_update` would on
            // the viewer when the sharer reports an active CLI agent.
            let view_id = view.id();
            CLIAgentSessionsModel::handle(ctx).update(ctx, |sessions, ctx| {
                sessions.set_session(
                    view_id,
                    CLIAgentSession {
                        agent: CLIAgent::Claude,
                        status: CLIAgentSessionStatus::InProgress,
                        session_context: CLIAgentSessionContext::default(),
                        input_state: CLIAgentInputState::Closed,
                        listener: None,
                        plugin_version: None,
                        remote_host: None,
                        draft_text: None,
                        custom_command_prefix: None,
                        should_auto_toggle_input: false,
                    },
                    ctx,
                );
            });

            view.maybe_show_use_agent_footer_in_blocklist(ctx);

            let model = view.model.lock();
            assert!(
                view.should_render_use_agent_footer(&model, ctx),
                "footer should render for viewer of shared cloud agent session with CLI agent",
            );
            let active_block_index = model.block_list().active_block_index();
            let rendered_footer_view_id = model
                .block_list()
                .last_non_hidden_rich_content_block_after_block(Some(active_block_index))
                .map(|(_, item)| item.view_id);
            assert_eq!(rendered_footer_view_id, Some(view.use_agent_footer.id()));
        });
    })
}
