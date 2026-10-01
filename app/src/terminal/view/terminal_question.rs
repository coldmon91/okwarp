use warp_core::{features::FeatureFlag, send_telemetry_from_ctx};
use warpui::ViewContext;

use crate::{
    ai::{agent::conversation::AIConversationId, blocklist::agent_view::AgentViewEntryOrigin},
    server::telemetry::TelemetryAgentViewEntryOrigin,
    terminal::TerminalView,
    TelemetryEvent,
};

impl TerminalView {
    /// Whether switching the input to agent mode should open a terminal question: only from the
    /// local terminal, never for read-only viewers or cloud agent panes.
    pub(super) fn can_open_terminal_question(&self, ctx: &ViewContext<Self>) -> bool {
        FeatureFlag::AgentView.is_enabled()
            && !self.agent_view_controller.as_ref(ctx).is_active()
            && !self.model.lock().shared_session_status().is_viewer()
            && !self.ambient_agent_view_model.as_ref(ctx).is_ambient_agent()
    }

    /// Opens a one-shot question about the terminal in a new conversation. The inline agent view
    /// keeps the terminal on screen; only the input switches to agent mode.
    pub(super) fn open_terminal_question(&mut self, ctx: &mut ViewContext<Self>) {
        let origin = AgentViewEntryOrigin::TerminalQuestion;
        if !self
            .ai_context_model
            .as_ref(ctx)
            .can_start_new_conversation()
        {
            self.show_error_toast(
                "Cannot ask a question while agent is monitoring a command.".to_string(),
                ctx,
            );
            return;
        }

        let result = self.agent_view_controller.update(ctx, |controller, ctx| {
            controller.try_enter_inline_agent_view(None, origin, ctx)
        });
        match result {
            Ok(_) => {
                send_telemetry_from_ctx!(
                    TelemetryEvent::AgentViewEntered {
                        origin: TelemetryAgentViewEntryOrigin::from(origin),
                        did_auto_trigger_request: false,
                    },
                    ctx
                );
            }
            Err(e) => {
                log::error!("Failed to open terminal question: {e:?}");
                self.show_error_toast(e.to_string(), ctx);
            }
        }
        self.redetermine_global_focus(ctx);
    }

    /// Returns the input to the terminal once the turn answering a terminal question is done.
    pub(super) fn finish_terminal_question(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ViewContext<Self>,
    ) {
        let agent_view_state = self.agent_view_controller.as_ref(ctx).agent_view_state();
        if agent_view_state.is_terminal_question()
            && agent_view_state.active_conversation_id() == Some(conversation_id)
        {
            self.agent_view_controller.update(ctx, |controller, ctx| {
                controller.exit_agent_view(ctx);
            });
        }
    }
}
