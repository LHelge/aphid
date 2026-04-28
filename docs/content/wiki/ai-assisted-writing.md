---
title: AI-Assisted Writing
category: Customization
tags:
  - guide
---

If you use an AI coding agent to help write content — whether that is GitHub Copilot, Claude Code, Cursor, Windsurf, or another tool — you can improve the results by giving the agent context about how aphid sites work. This page provides a ready-made instruction block you can drop into your project.

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
Optional: `updated`, `image`, `description`, `tags` (list of strings).
The slug must be unique across all content. Use lowercase words separated by hyphens.
Filename pattern: `YYYY-MM-DD_slug.md`. Posts live at `/blog/<slug>/`.

## Wiki pages (`content/wiki/`)

All frontmatter fields are optional: `title`, `category`, `created`, `updated`, `tags`.
If `title` is omitted the filename stem is used. `category` groups pages on the wiki index.
Wiki pages live at `/wiki/<stem>/`.

## Standalone pages (`content/pages/`)

Required frontmatter: `title`. Optional: `order` (sort position in nav, lower = earlier).
Pages live at `/<stem>/`.

## Heading rules

The page title comes from frontmatter and is rendered as `<h1>` by the template. The markdown
pipeline shifts all heading levels up by one, so:

- Use `#` for top-level sections (becomes `<h2>`)
- Use `##` for subsections (becomes `<h3>`), and so on
- Never use `#` for the page title — that comes from frontmatter

## Wiki-links

Cross-link to any other page with `[[page-slug]]` or `[[page-slug|Display text]]`. The slug
is the filename without the `.md` extension. Wiki-links resolve across blog, wiki, and pages —
any slug that exists anywhere in `content/` is a valid target. Check what pages exist before
linking.

## Images and static files

Place files in `static/` and reference them with absolute paths:
`![alt](/static/images/photo.png)`.

## Supported markdown extensions

Tables, strikethrough (`~~text~~`), task lists (`- [x]`), footnotes (`[^1]`), and fenced code
blocks with syntax highlighting (specify the language after the opening fence).

## Writing style

- Blog posts: open with a concise introduction, use `#` sections, link to wiki pages where
  relevant, keep `description` to one or two sentences.
- Wiki pages: neutral reference tone, start with a summary paragraph, cross-link liberally.
- Keep content files focused — if a topic grows large, split it into its own page and link.
````

See also: [[markdown]], [[frontmatter]], [[wiki-links]].
