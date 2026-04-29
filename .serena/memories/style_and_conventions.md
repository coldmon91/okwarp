# Style And Conventions

- Rust codebase; follow project `rustfmt.toml` and run `cargo fmt` after code changes.
- User AGENTS.md rules: use `fd` instead of `find`, `rg` instead of `grep`; keep Korean answers evidence-based and avoid overclaiming; keep lines under 130 chars.
- Rust rules from AGENTS.md: no `unwrap()`/`expect()` in production code unless unavoidable; prefer safe Rust; use current Rust edition conventions; common behavior should be extracted into reusable functions/modules.
- Code organization: new modules/files should be separated by responsibility and kept focused.
- Network abstractions: prefer existing `ServerApi`, `http_client::Client`, GraphQL operation modules, and `websocket::WebSocket` patterns rather than ad hoc `reqwest`/socket calls.