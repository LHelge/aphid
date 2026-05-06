---
title: Themes
category: Customization
tags:
  - reference
---

A theme is a directory containing HTML templates and optional static files. Point `aphid.toml` at it with `theme_dir`:

```toml
theme_dir = "mytheme"
```

All paths are relative to the working directory where `aphid` is invoked. If `theme_dir` is omitted, the embedded default theme is used.

# Directory layout

```
mytheme/
  theme.toml          ← required metadata
  templates/          ← required; all .html templates go here
    base.html
    home.html
    blog_post.html
    blog_index.html
    wiki_page.html
    wiki_index.html
    page.html
    tag.html
    tags_index.html
    404.html
  static/             ← optional; copied to output static/
    css/
    js/
```

# theme.toml

```toml
name = "mytheme"
version = "0.1.0"
description = "Optional description."
```

`name` and `version` are required; `description` is optional.

# Templates

Templates use [Tera](https://keats.github.io/tera/) syntax — a Jinja2-style engine. The typical pattern is a `base.html` layout that other templates extend:

```html
{# base.html #}
<!DOCTYPE html>
<html>
<head><title>{% block page_title %}{{ site_title }}{% endblock %}</title></head>
<body>
  {% block content %}{% endblock %}
</body>
</html>
```

```html
{# blog_post.html #}
{% extends "base.html" %}
{% block content %}
  <h1>{{ title }}</h1>
  {{ content | safe }}
{% endblock %}
```

The `content` variable holds rendered HTML — always use the `| safe` filter to prevent double-escaping.

## Variables available in every template

These come from `base.html` and are available in all templates via inheritance:

| Variable | Type | Description |
|----------|------|-------------|
| `site_title` | string | From `title` in `aphid.toml` |
| `base_url` | string | From `base_url` in `aphid.toml` |
| `version` | string | The `aphid` binary version |
| `nav_pages` | list | Standalone pages sorted by `order`; each has `title` and `url` |
| `socials` | list | Social links from `aphid.toml`; each has `platform` and `url` |
| `favicon_tags` | string | HTML `<link>` tags for favicons (empty if no favicon configured). Render with `{{ favicon_tags \| safe }}` |
| `feed_atom_url` | string | Absolute URL to the Atom feed (`/feed.xml`) |
| `feed_rss_url` | string | Absolute URL to the RSS feed (`/rss.xml`) |

## Universal page variables

These appear on every page template (`blog_post.html`, `wiki_page.html`, `page.html`):

| Variable | Type | Description |
|----------|------|-------------|
| `title` | string | Page title — from frontmatter, or for wiki pages the slug-derived title when frontmatter omits one |
| `url` | string | Clean URL, e.g. `/blog/my-post/` |
| `content` | string | Rendered HTML body |
| `toc` | list | Heading entries; each has `level`, `text`, and `id`. Always present (may be empty) |
| `contains_mermaid` | bool | `true` when the body contains at least one ` ```mermaid ` block — gate the Mermaid runtime on this. See [Mermaid diagrams](#mermaid-diagrams) |

## blog_post.html

Universal page variables, plus:

| Variable | Type | Description |
|----------|------|-------------|
| `author` | string | From frontmatter |
| `image` | string? | Hero/headline image path or URL, from frontmatter |
| `description` | string? | Short summary, from frontmatter |
| `created` | string | Publication date, formatted `YYYY-MM-DD` |
| `updated` | string? | Last-edited date |
| `tags` | list | Each tag has `name` and `slug` |
| `newer_post` | object? | Adjacent post one step newer in the feed, or `null` on the newest post. Same shape as the post entries on `blog_index.html`. |
| `older_post` | object? | Adjacent post one step older in the feed, or `null` on the oldest post. Same shape. |

## wiki_page.html

Universal page variables, plus:

| Variable | Type | Description |
|----------|------|-------------|
| `category` | string | Category for this page. Falls back to `wiki_default_category` (default `"Other"`) when frontmatter omits it, so always non-empty |
| `backlinks` | list | Pages that link here via `[[wiki-link]]`. Each has `title` and `url` |
| `wiki_categories` | list | All wiki pages grouped by category — for rendering a sidebar with the current page highlighted (compare `page.url == url`). Each entry has `name` (string) and `pages` (list of `{title, url}`). Named categories come first in `wiki_categories` order, then alphabetical; the default catch-all group sorts last. |

## page.html

Just the universal page variables — no extras.

## home.html

Renders the site root (`/index.html`). Receives the post list plus an optional rendered `home` block from `content/home.md` (see [[configuration]]).

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list | All blog posts — see the post entry shape below |
| `home` | object? | Present when `content/home.md` exists. Has `content` (string, the rendered HTML — pass through `\| safe`). |
| `contains_mermaid` | bool | `true` when `home.md` contains at least one ` ```mermaid ` block. See [Mermaid diagrams](#mermaid-diagrams) |

## blog_index.html

Renders one page of the blog listing. Page 1 lives at `/blog/`; subsequent pages live at `/blog/page/2/`, `/blog/page/3/`, … See [[pagination]] for the full mechanics.

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list | Posts on the current page — see the post entry shape below |
| `pagination` | object? | Pagination state, or `null` when the entire listing fits on one page. See [[pagination]]. |

### Post entry shape

Each entry in `posts` (and on `tag.html`) has:

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Post title |
| `url` | string | Clean URL, e.g. `/blog/my-post/` |
| `created` | string? | Publication date, formatted `YYYY-MM-DD` |
| `image` | string? | Path or URL of the hero image, from frontmatter |
| `description` | string? | Short summary, from frontmatter |
| `tags` | list | Each tag has `name` and `slug` |

## wiki_index.html

| Variable | Type | Description |
|----------|------|-------------|
| `categories` | list | All wiki pages grouped by category. Each entry has `name` (string) and `pages` (list of `{title, url}`). Named categories come first in `wiki_categories` order, then alphabetical; the default catch-all group (`wiki_default_category`, default `"Other"`) sorts last. |

## tag.html

Renders one page of a tag's post listing. Page 1 lives at `/tags/{slug}/`; subsequent pages live at `/tags/{slug}/page/2/`, etc.

| Variable | Type | Description |
|----------|------|-------------|
| `tag` | string | Tag display name |
| `tag_slug` | string | URL-safe slug |
| `posts` | list | Tagged posts on the current page; each has `title`, `url`, `created?` |
| `pagination` | object? | Pagination state, or `null` when the entire tag fits on one page. See [[pagination]]. |

## tags_index.html

| Variable | Type | Description |
|----------|------|-------------|
| `tags` | list | All tags; each has `name`, `slug`, and `count` |

## 404.html

No additional variables beyond the site-level ones.

# Static files

Files under `mytheme/static/` are copied to the output's `static/` directory before any user static files. If a file exists in both the theme and the user's `static_dir`, the user's file wins.

Reference theme assets with an absolute path in templates:

```html
<link rel="stylesheet" href="/static/css/theme.css">
```

## Syntax highlighting

Code blocks are highlighted with CSS classes prefixed `hl-` (e.g. `hl-keyword`, `hl-string`, `hl-comment`). Your theme stylesheet must provide rules for these classes — otherwise code blocks will render in a single color. The default theme ships with [Catppuccin Mocha](https://catppuccin.com/) colors as a reference.

## Mermaid diagrams

` ```mermaid ` fenced blocks are emitted as `<pre class="mermaid">…</pre>` — they need a client-side runtime to render. `aphid` bundles `mermaid.min.js` and writes it to `/static/js/mermaid.min.js` on every build, but **loading and initialising it is the theme's responsibility**: `base.html` must include the script and call `mermaid.initialize(...)` so a theme without diagram support can skip the payload entirely.

Each page context exposes a `contains_mermaid` boolean (`true` when the body has at least one mermaid block). Gate the runtime on this so pages without diagrams don't pay the download cost:

```html
{% if contains_mermaid %}
<script src="/static/js/mermaid.min.js"></script>
<script>mermaid.initialize({ startOnLoad: true });</script>
{% endif %}
```

Pass any extra options (theme colors, flowchart config, …) into `mermaid.initialize`. To match diagrams to your site palette, set `theme: 'base'` and override `themeVariables`:

```html
{% if contains_mermaid %}
<script src="/static/js/mermaid.min.js"></script>
<script>
  mermaid.initialize({
    startOnLoad: true,
    theme: 'base',
    themeVariables: {
      darkMode: true,
      background: '#1e1e2e',
      primaryColor: '#313244',
      primaryTextColor: '#cdd6f4',
      primaryBorderColor: '#cba6f7',
      lineColor: '#b4befe',
      textColor: '#cdd6f4',
    },
  });
</script>
{% endif %}
```

See the [Mermaid theming docs](https://mermaid.js.org/config/theming.html) for the full set of `themeVariables` keys (sequence, flowchart, class, state, and Gantt diagrams each expose their own colour knobs). The docs theme's `base.html` is a worked example using a Catppuccin Mocha palette.
