# Frequently Asked Questions

This FAQ covers common questions about Swarf, a fork of the open-source Warp client codebase.
For contribution guidance, see [CONTRIBUTING.md](CONTRIBUTING.md). For engineering notes, see
[WARP.md](WARP.md).

> [!NOTE]
> Swarf is not an official Warp Technologies release. Some internal names may still reference
> upstream Warp while this fork is being adapted.

## Contributing

### How do I contribute?

Start with a GitHub issue in this repository. For bugs, include reproduction steps, expected behavior,
actual behavior, your Swarf version or commit, and your operating system.

For feature requests, describe the user-facing problem before proposing an implementation. Keep product
removal, refactoring, and new feature work in separate changes when practical.

### How do I build and run Swarf from source?

```bash
./script/bootstrap   # platform-specific setup
./script/run         # build and run the app
./script/presubmit   # format, lint, and test checks
```

macOS, Linux, and Windows are supported by the upstream client codebase, but fork-specific behavior should
be verified against the current source tree.

### Can I use my own coding agent with Swarf?

Where the codebase supports external CLI agents, you can bring tools such as Codex, Claude Code,
Gemini CLI, or similar command-line agents. Hosted or commercially managed agent features from the
upstream project may not be present in this fork.

## Project Scope

### Is Swarf fully independent from upstream Warp?

Not yet. Swarf is a fork of the open-source Warp client codebase, so source files, crate names,
module names, tests, and older specs may still use upstream terminology.

Treat visible branding, support links, update endpoints, telemetry, authentication, billing, and
cloud-connected behavior as areas that need explicit review before redistribution.

### What changed from upstream?

The project intent is to keep the terminal and local development workflow while removing or avoiding
commercially oriented product surfaces. This FAQ does not guarantee that every upstream integration has
already been removed.

### Can I run Swarf without signing in?

Some client functionality may work locally, while cloud-connected features may depend on upstream server
APIs or fork-specific replacements. Verify behavior in the current build before making user-facing claims.

## Licensing

### What licenses apply?

The upstream repository uses this license split:

- The `warpui_core` and `warpui` crates are licensed under the [MIT license](LICENSE-MIT).
- The rest of the code is licensed under the [AGPL v3](LICENSE-AGPL).

Swarf keeps those license files. Review the license texts directly before redistributing modified builds,
hosting modified services, or combining this code with other projects.

### Can someone fork Swarf?

The AGPL and MIT license files define the legal permissions and obligations. In general, preserve license
notices, keep source availability obligations in mind, and avoid implying endorsement by upstream Warp
Technologies.

## Help and Security

### Where do I get help?

Use this repository's issue tracker or maintainer contact process. Avoid directing Swarf users to upstream
Warp product support unless the issue is specifically about the upstream project.

### How do I report a security vulnerability?

Do not open a public issue for a vulnerability. Follow [SECURITY.md](SECURITY.md) and contact the Swarf
maintainers privately using the address or process listed there.
