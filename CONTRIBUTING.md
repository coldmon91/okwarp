# Contributing To Swarf

Thanks for helping improve Swarf.

Swarf is a community fork of the open-source Warp client codebase. This contribution guide intentionally
avoids upstream Warp operational processes, support channels, and automated review services unless they are
explicitly adopted by this fork later.

## Before You Start

- Search this repository's issues before opening a new one.
- Keep each change focused on a single behavior, fix, or cleanup.
- Do not add commercial-only product surfaces.
- Preserve upstream copyright and license notices.
- Treat authentication, networking, billing, telemetry, and cloud-connected code paths as sensitive areas.

## Bug Reports

A useful bug report includes:

- A clear summary of the problem.
- Steps to reproduce.
- Expected and actual behavior.
- Swarf version or commit.
- Operating system and shell details.
- Logs, screenshots, or recordings when relevant.

Do not include secrets, tokens, private keys, or private customer data in public issues.

## Feature Requests

Feature requests should describe the user-facing problem before the implementation.

- Explain who needs the feature.
- Describe the current behavior and why it falls short.
- Describe the desired behavior.
- Note compatibility, security, privacy, or licensing constraints.

Large or ambiguous features should start with a short product and technical spec before implementation.

## Development Setup

```bash
./script/bootstrap   # platform-specific setup
./script/run         # build and run the app
./script/presubmit   # format, lint, and tests
```

See [WARP.md](WARP.md) for engineering notes inherited from the upstream codebase.

## Code Style

- Follow the existing Rust style and project structure.
- Run `cargo fmt` before submitting Rust changes.
- Add or update tests when changing behavior.
- Keep user-facing branding aligned with Swarf unless an upstream name is required for compatibility.

## Security Issues

Do not open public issues for vulnerabilities. See [SECURITY.md](SECURITY.md).
