---
title: CLI reference
category: Getting Started
tags:
  - reference
  - cli
---

The `aphid` binary has several subcommands and a small set of flags. Running `aphid` with no subcommand starts the dev server on port 3000 — the most common case while writing content.

# Synopsis

```
aphid [--config <path>] [<command>] [command flags]
```

# Commands

## `aphid new`

Create a new aphid site in a new directory. Generates a minimal but complete site structure: config file, example blog post, wiki page, about page, home page, static directory, and `.gitignore`.

```
aphid new <name> [--agent [TOOL]]
```

The directory name is converted into the site title (`my-cool-blog` becomes "My Cool Blog"). Fails if the directory already exists.

| Flag | Description |
|------|-------------|
| `--agent [TOOL]` | Also write AI-agent instruction files. Pass `claude`, `copilot`, or `codex`; omit the value for a generic `AGENTS.md`. See [[#aphid-agent]] for details. |

## `aphid init`

Initialize an aphid site in an existing directory (or the current directory). Creates the same files as `aphid new`, but does not create a new parent directory.

```
aphid init [path] [--agent [TOOL]]
```

| Argument | Default | Description |
|----------|---------|-------------|
| `path` | `.` | Directory to initialize |

| Flag | Description |
|------|-------------|
| `--agent [TOOL]` | Also write AI-agent instruction files. Pass `claude`, `copilot`, or `codex`; omit the value for a generic `AGENTS.md`. See [[#aphid-agent]] for details. |

Fails if the target directory already contains an `aphid.toml`.

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

## `aphid blog new`

Create a new blog post in the current site. The title is used to derive the slug and filename. Today's date is used for both the filename prefix and the `created` frontmatter field.

```
aphid blog new <title>
```

Example: `aphid blog new "My First Post"` creates `content/blog/2026-05-02_my-first-post.md`.

## `aphid wiki new`

Create a new wiki page in the current site.

```
aphid wiki new <title>
```

Example: `aphid wiki new "Architecture Overview"` creates `content/wiki/architecture-overview.md`.

## `aphid page new`

Create a new standalone page in the current site.

```
aphid page new <title>
```

Example: `aphid page new "Contact"` creates `content/pages/contact.md`.

## `aphid agent`

Write AI-agent instruction files for the current site, so a coding agent can author content or edit themes with full project context. The instruction text is embedded in the binary — no network or filesystem lookup is required.

```
aphid agent [TOOL]
```

| Argument | Default | Description |
|----------|---------|-------------|
| `TOOL` | `codex` | One of `claude`, `copilot`, `codex`. Omit for a generic `AGENTS.md` (the `codex` layout, which is also recognised by Aider, Goose, and current Cursor). |

For each tool, two scoped skills are written alongside the main instruction file: `aphid-content` (markdown, frontmatter, wiki-links, `aphid.toml`) and `aphid-theme` (Tera templates, template variables, static assets).

| Tool | Files written |
|------|---------------|
| `claude` | `CLAUDE.md`, `.claude/skills/aphid-content/SKILL.md`, `.claude/skills/aphid-theme/SKILL.md` |
| `copilot` | `.github/copilot-instructions.md`, `.github/instructions/aphid-content.instructions.md`, `.github/instructions/aphid-theme.instructions.md` |
| `codex` | `AGENTS.md`, `.agents/aphid-content.md`, `.agents/aphid-theme.md` |

Re-runnable after upgrading aphid: the **skill files are always overwritten** so they track the installed aphid version, while the **main instruction file** (`CLAUDE.md`, `.github/copilot-instructions.md`, or `AGENTS.md`) is preserved if it already exists — a warning is logged in that case. This lets you extend the main file with project-specific guidance without having it clobbered on re-run; delete the main file first if you want a clean regeneration.

The same logic is reachable from `aphid init --agent <TOOL>` and `aphid new <name> --agent <TOOL>`, so first-time setup is a single command.

See [[ai-assisted-writing]] and [[ai-assisted-design]] for the prose versions of these files.

# Global flags

| Flag | Default | Description |
|------|---------|-------------|
| `--config`, `-c` | `aphid.toml` | Path to the site config file. May be passed before or after the subcommand. |
| `--version` | — | Print the binary version and exit |
| `--help`, `-h` | — | Print help text |

# Examples

```sh
aphid new my-blog                        # scaffold a new site in my-blog/
aphid new my-blog --agent claude         # scaffold + write Claude Code instructions
aphid init                               # scaffold in the current directory
aphid init --agent                       # scaffold + write a generic AGENTS.md
aphid init path/to/site                  # scaffold in a specific directory
aphid blog new "My First Post"           # create a new blog post
aphid wiki new "Architecture Overview"   # create a new wiki page
aphid page new "Contact"                 # create a new standalone page
aphid agent claude                       # write Claude Code instructions for this site
aphid agent                              # write a generic AGENTS.md
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
