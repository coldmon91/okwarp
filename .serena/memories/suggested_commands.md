# Suggested Commands

- Search files: `fd <pattern> <path>`
- Search contents: `rg -n <pattern> <path>`
- Build/run from source: `./script/bootstrap`, then `./script/run`
- Presubmit: `./script/presubmit`
- Local server development example from `WARP.md`: `SERVER_ROOT_URL=http://localhost:8082 WS_SERVER_URL=ws://localhost:8082/graphql/v2 cargo run --features with_local_server`
- Rust formatting should use the repo rustfmt settings, typically through `cargo fmt` or presubmit.
- Always wrap long-running commands with a timeout, e.g. `perl -e 'alarm shift; exec @ARGV' <seconds> <command> ...` or approved `timeout` commands when available.