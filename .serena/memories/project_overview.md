# Project Overview

- Project: Warp open-source client repository.
- Purpose: Agentic development environment / terminal client with local terminal, Warp Drive objects, AI/agent workflows, shared sessions, remote server support, and platform integrations.
- Tech stack: Rust workspace with `app` plus many `crates/*`; UI uses Warp's `warpui` / `warpui_core`; GraphQL operations are generated/typed via `cynic`; HTTP uses local `http_client` wrapper around `reqwest`; WebSocket uses local `websocket` crate over `async-tungstenite` plus `graphql-ws-client`.
- High-level structure: `app/src` contains product features and UI; `app/src/server` wraps Warp server APIs; `crates/graphql` defines typed GraphQL queries/mutations/subscriptions; `crates/warp_core` contains shared channel/config primitives; `crates/http_client` and `crates/websocket` abstract network transport; `crates/remote_server` supports SSH remote server installation/communication.
- There is no root `doc/` or `docs/` directory; root `README.md` and `WARP.md` are primary overview/developer docs.