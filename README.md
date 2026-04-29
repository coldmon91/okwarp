# Swarf

Swarf is a community fork of the open-source Warp client codebase.

This fork is being adapted to reduce or remove commercially oriented upstream product surfaces while
preserving the terminal-first development experience where the current source supports it.

> [!IMPORTANT]
> Swarf is not an official Warp Technologies release. Some source files, scripts, assets, and documentation
> may still contain legacy upstream references while this fork is being adapted.

## Scope

The current repository should be treated as an in-progress fork, not as a fully rebranded product.

- Terminal and local development workflows are the primary focus.
- Commercial, billing, hosted-agent, telemetry, and cloud-connected paths should be reviewed before use.
- Upstream compatibility names may remain where changing them would break existing code or persisted data.
- User-facing branding should avoid implying that Swarf is an official Warp release.

## Building Locally

To set up your environment and run Swarf from source:

```bash
./script/bootstrap   # platform-specific setup
./script/run         # build and run the app
./script/presubmit   # format, lint, and test checks
```

For advanced development workflows and Cargo commands, see [WARP.md](WARP.md). That file may still contain
upstream terminology, but it remains useful engineering context for this codebase.

## Development And Contribution

Contributions should preserve the goals of this fork.

- Keep patches focused.
- Avoid adding commercial-only product surfaces.
- Run relevant formatting, linting, and tests before submitting changes.
- Treat authentication, networking, billing, telemetry, and cloud-connected code paths as sensitive areas.
- Check whether an upstream reference is required for compatibility before renaming it.

The existing [CONTRIBUTING.md](CONTRIBUTING.md) describes the Swarf contribution baseline for this fork.

## Licensing

Swarf keeps the upstream licensing split:

- [MIT License](LICENSE-MIT): applies to the `warpui_core` and `warpui` crates.
- [AGPL v3 License](LICENSE-AGPL): applies to the remainder of the codebase.

Do not remove upstream copyright or license notices. Review the license texts directly before redistributing
modified builds or combining this code with other projects.

## Code Of Conduct

This repository includes [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Contributors are expected to follow it
when participating in project spaces.

## Open Source Ecosystem

Swarf builds on the same broad ecosystem used by the upstream client, including:

- [Tokio](https://github.com/tokio-rs/tokio)
- [NuShell](https://github.com/nushell/nushell)
- [Smol](https://github.com/smol-rs/smol)
- [Alacritty](https://github.com/alacritty/alacritty)
- [Fig Completion Specs](https://github.com/withfig/autocomplete)
- [FontKit](https://github.com/servo/font-kit)
- [Hyper HTTP library](https://github.com/hyperium/hyper)
- [Warp Server Framework](https://github.com/seanmonstar/warp)
- [Core-foundation](https://github.com/servo/core-foundation-rs)

See dependency manifests and license files for the full list of dependencies.
