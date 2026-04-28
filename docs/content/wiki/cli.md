---
title: CLI reference
category: Getting Started
tags:
  - reference
---

The `aphid` binary has two subcommands and a small set of flags. Running `aphid` with no subcommand starts the dev server on port 3000 — the most common case while writing content.

# Synopsis

```
aphid [--config <path>] [<command>] [command flags]
```

# Commands

## `aphid build`

Render every page into the output directory (default `./dist/`) and copy theme + user static files alongside. The output directory is wiped and recreated on every build, so it must not point at a directory containing files you want to keep. Broken `[[wiki-links]]` cause the build to fail and report every offending page.

| Flag | Default | Description |
|------|---------|-------------|
| `--output`, `-o` | `dist` | Directory to write the rendered site into |

This is the command for CI and one-shot deployments — see [[deployment]].

## `aphid serve`

Run a development server with file watching and WebSocket-driven live reload. Bare `aphid` (no subcommand) is shorthand for `aphid serve --port 3000`. See [[dev-server]] for the full description.

| Flag | Default | Description |
|------|---------|-------------|
| `--port`, `-p` | `3000` | TCP port to bind |

# Global flags

| Flag | Default | Description |
|------|---------|-------------|
| `--config`, `-c` | `aphid.toml` | Path to the site config file. May be passed before or after the subcommand. |
| `--version` | — | Print the binary version and exit |
| `--help`, `-h` | — | Print help text |

# Examples

```sh
aphid                                    # serve on :3000
aphid serve --port 8080                  # serve on :8080
aphid build                              # one-shot render into dist/
aphid build --output public              # render into public/
aphid --config docs/aphid.toml build     # render the docs site
aphid -c sub/aphid.toml                  # serve with a non-default config
```

# Logging

Logs are written to stderr via [tracing](https://docs.rs/tracing). Set `RUST_LOG` to control verbosity:

```sh
RUST_LOG=debug aphid serve     # verbose
RUST_LOG=warn aphid build      # warnings and errors only
```

The default level is `info`.

See also: [[dev-server]], [[configuration]], [[deployment]].
