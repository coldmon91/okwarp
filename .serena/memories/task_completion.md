# Task Completion Checklist

- Before code modification, explain the planned change direction and ask whether to proceed.
- After Rust code changes, run `cargo fmt` and the narrowest relevant tests/checks with a timeout.
- For broader or risky changes, run repo-prescribed checks such as `./script/presubmit` when feasible.
- Evaluate behavioral, performance, compatibility, and integration side effects before and after changes.
- Do not leave background processes running; stop any dev servers or spawned processes when work is complete.
- Delete temporary files/directories created during work, excluding MCP-managed temporary files.
- Do not revert unrelated user changes in the worktree.