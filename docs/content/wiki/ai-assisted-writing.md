---
title: AI-Assisted Writing
category: Customization
tags:
  - guide
  - ai
  - content
---

If you use an AI coding agent to help write content — whether that is GitHub Copilot, Claude Code, Cursor, Windsurf, or another tool — you can improve the results by giving the agent context about how aphid sites work. This page provides a ready-made instruction block you can drop into your project.

> [!TIP]
> Run `aphid agent claude` (or `copilot`, `codex`) to write these files for you in the right location with the right frontmatter. See [[cli#aphid-agent]] for the full description.

Most agent systems support some form of project-level instructions: a markdown file in the repository that the agent reads automatically. The exact file and location varies:

| Tool | File | Location |
|------|------|----------|
| GitHub Copilot | `copilot-instructions.md` | `.github/` |
| Claude Code | `CLAUDE.md` | project root |
| Cursor | `.cursorrules` | project root |
| Windsurf | `.windsurfrules` | project root |
| Aider | `.aider.conf.yml` | project root |

Check your tool's documentation for the precise format. The guidance below is written as plain markdown that you can paste into whichever file your tool expects.

# Instructions

Copy the block below into your tool's project-level instruction file:

````markdown
This project is a static site built with aphid. Content lives under `content/` in three
subdirectories:

- `content/blog/` — dated blog posts
- `content/wiki/` — reference/wiki pages
- `content/pages/` — standalone pages (About, Contact, etc.)

Every content file is Markdown with YAML frontmatter delimited by `---`.

## Blog posts (`content/blog/`)

Required frontmatter: `title`, `slug`, `author`, `created` (YYYY-MM-DD).
Optional: `updated`, `image`, `description`, `tags` (list of strings), `draft` (bool).
The slug must be unique across all content. Use lowercase words separated by hyphens.
Filename pattern: `YYYY-MM-DD_slug.md`. Posts live at `/blog/<slug>/`.

## Wiki pages (`content/wiki/`)

All frontmatter fields are optional: `title`, `category`, `created`, `updated`, `tags`,
`draft`. If `title` is omitted the filename stem is used. `category` groups pages on the
wiki index. Wiki pages live at `/wiki/<stem>/`.

## Standalone pages (`content/pages/`)

Required frontmatter: `title`. Optional: `order` (sort position in nav, lower = earlier),
`draft`. Pages live at `/<stem>/`.

## Site config (`aphid.toml`)

Top-level fields used in content:

- `title` (required) — site title.
- `base_url` (required) — canonical root URL, e.g. `https://example.com`.
- `description` — used in feeds and SEO.
- `source_dir` — content root (default `content`).
- `static_dir` — static files root (default `static`).
- `theme_dir` — theme directory (optional, defaults to the bundled theme).
- `wiki_categories` — list of category names in display order.
- `wiki_default_category` — fallback category name (default `"Other"`).
- `favicon`, `social_image` — site-wide assets.
- `feed_limit` (default 20), `posts_per_page` (default 10), `reading_wpm` (default 200).
- `[[authors]]` — array of `{ name, link?, email?, image? }`.
- `[[socials]]` — array of `{ platform, url }`.

## Heading rules

The page title comes from frontmatter and is rendered as `<h1>` by the template. The
markdown pipeline shifts all heading levels up by one, so:

- Use `#` for top-level sections (becomes `<h2>`)
- Use `##` for subsections (becomes `<h3>`), and so on
- Never use `#` for the page title — that comes from frontmatter

Custom heading IDs are supported with the `{#custom-id}` suffix after the heading text.

## Wiki-links

Cross-link to any other page with `[[slug]]`, `[[slug|Display text]]`, `[[slug#section]]`,
or `[[#section]]` (same-page anchor). The slug is the filename without the `.md` extension.
Wiki-links resolve across blog, wiki, and pages — any slug that exists anywhere in
`content/` is a valid target. Check what pages exist before linking.

Prefer `[[page#section]]` over hand-written `/wiki/page/#section` markdown links — the wiki
form is checked at build time.

## Images and static files

Place files in `static/` and reference them with absolute paths:
`![alt](/static/images/photo.png)`. The user `static/` directory wins over theme static
files of the same path.

## Supported markdown extensions

- Tables, strikethrough (`~~text~~`), task lists (`- [x]`), footnotes (`[^1]`).
- Smart punctuation — straight quotes and `--`/`---` get converted to typographic forms at
  render time. Write naturally; don't hand-type `"`, `—`, etc.
- Fenced code blocks with syntax highlighting (specify the language after the opening
  fence). Highlighting emits CSS classes prefixed `hl-`.
- Mermaid diagrams via fenced ` ```mermaid ` blocks — rendered as `<pre class="mermaid">`
  and picked up by the bundled `mermaid.min.js`.
- GitHub-style alerts: `> [!NOTE]`, `> [!TIP]`, `> [!IMPORTANT]`, `> [!WARNING]`,
  `> [!CAUTION]`.

## Writing style

- Blog posts: open with a concise introduction, use `#` sections, link to wiki pages where
  relevant, keep `description` to one or two sentences.
- Wiki pages: neutral reference tone, start with a summary paragraph, cross-link liberally.
- Keep content files focused — if a topic grows large, split it into its own page and link.
````

See also: [[markdown]], [[frontmatter]], [[wiki-links]].
