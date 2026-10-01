# Shared Agent View ("함께 보기") Design

Status: Approved design (2026-10-01), updated with implementation decisions and the terminal question decision

## Goal

Agent conversations no longer switch the terminal into a separate full-screen view.
Terminal command blocks and agent conversation blocks live in one scroll, in chronological order, at all times.
The agent automatically receives the commands and output the user ran in the terminal since the last query.

Cmd-I is a one-shot question about the current terminal, not an agent conversation.
Nothing covers the terminal while the question is open; only the input switches to agent mode.
The answer stays in the scroll and the input returns to the terminal once the answer is done.

Out of scope:
- Multi-turn agent conversations move to a separate window (separate design).
- Removing natural-language autodetection entirely (separate design).
  Until it lands, autodetection keeps working, but its effect is only that a conversation starts without a screen switch.

## 1. Display and entry

### Visibility

The `AgentViewDisplayMode::FullScreen` variant keeps its name (51 references; renaming would amplify upstream merge conflicts).
Its doc comment is updated to describe the shared semantics: "an agent conversation is active and owns the input".

| Location | Before | After |
|---|---|---|
| `Block::should_hide_block` (`app/src/terminal/model/block.rs`) | FullScreen hides blocks outside the conversation; Inactive/Inline hides agent-run blocks | Agent view state never hides a block; only `hidden` blocks are hidden |
| `RichContentItem::should_hide_for_agent_view_state` (`app/src/terminal/model/blocks.rs`) | Hides by conversation membership | Always visible, except content created while in Inline display mode over a long-running command (it is rendered over the long-running block) |
| `AIBlock::is_hidden` (`app/src/ai/blocklist/block.rs`) | Active agent view shows only its own conversation's AI blocks; no agent view shows only passive ones | Only hidden exchanges (passive features' internal requests) are hidden; answers stay after the question or conversation closes |
| `ViewportIter` (`app/src/terminal/block_list_viewport.rs`) | Fullscreen yields only its own conversation's rich content; otherwise only terminal-owned rich content (the space of skipped items stays reserved, leaving a blank gap) | Yields every rich content item that is not `should_hide` |
| Separators/banners in fullscreen (`blocks.rs`) | Hidden in fullscreen | Always visible |
| Enter (`app/src/terminal/view.rs`) | Saves scroll position, scrolls to bottom, inserts zero-state block | No scroll save; scrolls to bottom; zero-state block kept (it hides itself once the conversation has an exchange and is removed on exit) |
| Exit (`view.rs`) | Restores scroll position, inserts `AgentViewEntryBlock` | No scroll restore; entry block only for LRC conversations |

Unchanged: agent input footer, slash menus, model selector, double-Esc/Ctrl-C exit confirmation, LRC (long-running command) Inline mode.

To keep long-running-command Inline content out of the scroll, a `RichContentItem` records whether it was created while the agent view was Inline over a long-running command (`is_inline_agent_view_content`); terminal question answers are not marked.
That content is shown only while its own conversation is open in the shared agent view, so the LRC entry block still leads to it.
Entry blocks inserted for conversations restored on startup are kept; they are the way to continue a past conversation after a restart.

### Terminal question (Cmd-I)

The full-screen agent view covers the terminal (a tinted overlay over the whole terminal view, the zero-state block, a forced pinned-to-bottom input), so Cmd-I uses the Inline display mode instead.

| Step | Behavior |
|---|---|
| Open | Cmd-I in terminal mode starts a new conversation in the Inline display mode with origin `AgentViewEntryOrigin::TerminalQuestion` (`TerminalView::open_terminal_question`). It never continues an earlier question. Not for shared-session viewers or cloud agent panes. |
| Screen | No zero-state block, no inline agent view header (that header describes a long-running command). Only the input background is tinted. |
| Ask | The prompt is sent with the auto-attached terminal context (section 2). |
| Answer | Answer blocks are not inline agent view content (`AgentViewState::is_terminal_question`), so they stay in the scroll during and after the question. |
| Return | When the turn finishes (complete, error, or cancelled), the question closes and the input returns to the terminal (`TerminalView::finish_terminal_question`). A turn waiting on user approval keeps the question open. |
| Cancel | Esc or switching the input back to terminal mode closes the question; an unused empty conversation is removed. |

The idle prompt block is not tagged with the question's conversation, because a question runs no command.
Cmd-I over a long-running command keeps the existing tag-in behavior (origin `LongRunningCommand`).

### Other entry paths

- With `AgentView` enabled, the input model cannot lock AI mode outside a conversation (`BlocklistAIInputModel::set_input_config_internal`), so Cmd-I in terminal mode used to do nothing; the terminal question is what Cmd-I now opens.
- Prompts submitted with Cmd-Enter or `/agent` open the full agent view and continue the most recent live conversation in this terminal (`TerminalView::conversation_to_continue`); `/new` starts a fresh one. These stay until the separate conversation window replaces them.
- A prompt submitted from the input is sent immediately; the former "press Enter again to send" confirmation is gone (`AgentViewEntryOrigin::Input` always auto-triggers).
- Kept (explicit user choice): Cmd-Enter, `/agent`, conversation list, opening a project, new-tab default session mode.
- Removed (implicit entry):
  - Pasting an image never enters a conversation. Images attach only while a conversation or question is active (after Cmd-I); they are sent with the next prompt.
  - Prompt suggestions are offered and accepted only inside an active conversation.

## 2. Automatic terminal context

| Item | Rule |
|---|---|
| Collected | Blocks the user ran (not part of an agent interaction), regardless of input mode |
| Window | Since the last submitted user query; cleared on submit (existing `reset_context_to_default`) |
| Not cleared | Entering/exiting the agent view no longer clears the list |
| Cap | Most recent 5 blocks; oldest dropped first |
| Secrets | Auto-attached output forces secret obfuscation; manually attached blocks keep following user settings |
| Dedup | A block both manually and automatically attached is sent once (existing behavior) |
| BYOK output length | 4,000-char budget kept, as head 1/4 + tail 3/4 with a truncation marker; applies to all blocks and the running command |
| BYOK total context | The 12,000-char budget for the whole rendered context also keeps head 1/4 + tail 3/4, so the most recent blocks (rendered last) survive |
| UI | No new indicator; attached blocks are visible right above in the scroll |

## 3. Testing

| Kind | Location | Checks |
|---|---|---|
| Update existing | `app/src/terminal/model/blocks_test.rs` | Entering does not hide terminal blocks; exiting keeps agent blocks; `hidden` blocks stay hidden |
| New unit | `blocks_test.rs` | Inline-mode (long-running command) rich content stays hidden; terminal question answers stay visible after the question closes; the idle prompt block stays untagged |
| New unit | `app/src/ai/blocklist/context_model_tests.rs` | Cap of 5 drops oldest |
| New unit | `app/src/ai/agent/api/byok_openai.rs` | Head/tail truncation over the budget; short output unchanged; multibyte characters |
| View | `app/src/terminal/view_test.rs` | No entry block on exit for a non-LRC conversation; Cmd-I opens a question without zero-state block or header; only the question's own finished turn closes it; the next Cmd-I starts a new conversation; the answer block stays visible after the question closes and while the next question is open, and the viewport yields it for rendering |
| Input | `app/src/terminal/input_test.rs` | Image paste is ignored in terminal mode; Cmd-I opens a terminal question and switches the input to agent mode; images then attach; switching back to terminal mode closes the question |
| Manual | Running app | `cargo build` failure, then Cmd-I question: the screen stays, the BYOK answer points at the error, and the input returns to terminal mode |

Run with `cargo nextest` plus the presubmit checks (fmt, clippy).

Not covered by automated tests (needs a conversation with exchanges or completed shell blocks in a test harness): closing the question from the real end-of-turn event, conversation continuation after Esc, auto-attach of pre-entry blocks, exclusion of agent-run blocks, clearing after submit, and secret obfuscation of auto-attached output.
These are part of the manual check.
