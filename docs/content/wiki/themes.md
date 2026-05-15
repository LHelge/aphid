---
title: Themes
category: Customization
tags:
  - reference
  - themes
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
| `site_description` | string? | From `description` in `aphid.toml`. Used as the OpenGraph description fallback on pages without their own |
| `social_image_url` | string? | Absolute URL for the site-wide default OpenGraph / Twitter card image — from `social_image` in `aphid.toml`. `None` when no `social_image` is configured |
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
| `url` | string | Root-relative URL, e.g. `/blog/my-post/` |
| `canonical_url` | string | Fully-qualified URL of this page (`base_url` joined with `url`). Render directly with `{{ canonical_url }}` for OpenGraph tags and similar — never concatenate URLs by hand in templates |
| `content` | string | Rendered HTML body |
| `toc` | list | Heading entries; each has `level`, `text`, and `id`. Always present (may be empty) |
| `contains_mermaid` | bool | `true` when the body contains at least one ` ```mermaid ` block — gate the Mermaid runtime on this. See [Mermaid diagrams](#mermaid-diagrams) |

## blog_post.html

Universal page variables, plus:

| Variable | Type | Description |
|----------|------|-------------|
| `author` | object | Author metadata resolved from config. Has `.name` (string), `.link` (string?), `.image` (string?). If the frontmatter author name matches a `[[authors]]` entry in `aphid.toml`, `.link` and `.image` are populated from config; otherwise only `.name` is set. `.link` uses the author's `link` field if set, falling back to `mailto:{email}` when only `email` is configured. |
| `image` | string? | Hero/headline image path or URL, from frontmatter |
| `og_image` | string? | Absolute URL of the post's OpenGraph / Twitter card image — `image` resolved against `base_url`, or the site `social_image_url` fallback. `None` when neither is set |
| `description` | string? | Short summary, from frontmatter |
| `created` | string | Publication date, formatted `YYYY-MM-DD` |
| `updated` | string? | Last-edited date |
| `reading_time_minutes` | integer | Rough reading-time estimate for the body, in minutes (rounded up, minimum 1). Render as e.g. `{{ reading_time_minutes }} min read` |
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
| `popular_tags` | list | Every tag used across blog and wiki content, sorted by descending count then ascending name. Each entry has `name`, `slug`, and `count`. Counts match the `/tags/` index. Slice with `\| slice(end=N)` in Tera if you want a top-N cloud. |

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
| `reading_time_minutes` | integer | Rough reading-time estimate for the body, in minutes (rounded up, minimum 1) — same value as on the post template itself |
| `tags` | list | Each tag has `name` and `slug` |

## wiki_index.html

| Variable | Type | Description |
|----------|------|-------------|
| `categories` | list | All wiki pages grouped by category. Each entry has `name` (string) and `pages` (list of `{title, url}`). Named categories come first in `wiki_categories` order, then alphabetical; the default catch-all group (`wiki_default_category`, default `"Other"`) sorts last. |

## tag.html

Renders one page of a tag's post listing. Page 1 lives at `/tags/{slug}/`; subsequent pages live at `/tags/{slug}/page/2/`, etc.

Posts are split into `blog_posts` and `wiki_pages` so themes can render them as separate sections. Pagination operates over the combined list (same `posts_per_page` as before), and each page's slice is partitioned by kind — either list can be empty on any given page, so guard both with a length check.

| Variable | Type | Description |
|----------|------|-------------|
| `tag` | string | Tag display name |
| `tag_slug` | string | URL-safe slug |
| `blog_posts` | list | Blog posts in this page's slice, in feed order. Post entry shape (see above). Empty when the slice has no blog posts |
| `wiki_pages` | list | Wiki pages in this page's slice. Same shape. Empty when the slice has no wiki pages |
| `pagination` | object? | Pagination state, or `null` when the entire tag fits on one page. See [[pagination]]. |

## tags_index.html

| Variable | Type | Description |
|----------|------|-------------|
| `tags` | list | All tags; each has `name`, `slug`, and `count` |

## 404.html

Renders to `dist/404.html` (and is served by `aphid serve` for any unknown path). Optionally receives content from `content/404.md`; if the file isn't present the variable is `null` and the template's hardcoded fallback runs.

| Variable | Type | Description |
|----------|------|-------------|
| `not_found` | object? | Present when `content/404.md` exists. Has `content` (string, the rendered HTML — pass through `\| safe`). |
| `contains_mermaid` | bool | `true` when `404.md` contains at least one ` ```mermaid ` block. See [Mermaid diagrams](#mermaid-diagrams) |

Templates carry the visual structure (the big "404" hero); the message is the author's job, written in `content/404.md`. Without `404.md` the page renders just the hero:

```jinja2
<div class="error-code">404</div>
{% if not_found %}
{{ not_found.content | safe }}
{% endif %}
```

# Social meta tags

The bundled themes' `base.html` emits OpenGraph and Twitter card meta tags in the `<head>` of every page, derived from context variables. Custom themes get the same behaviour by inheriting from `base.html`. Two blocks are exposed for overrides:

| Block | Default | Override on |
|-------|---------|-------------|
| `og_type` | `"website"` | `blog_post.html` → `"article"` |
| `article_meta` | empty | `blog_post.html` → `article:published_time`, `article:modified_time`, `article:author`, `article:tag` |

Tag content comes from these context fields:

- `og:title` / `twitter:title` — page `title`, falling back to `site_title`
- `og:description` / `twitter:description` / `<meta name="description">` — page `description`, falling back to `site_description`
- `og:url` — page `canonical_url` (only emitted when the page exposes one)
- `og:image` / `twitter:image` — blog post `og_image`, falling back to `social_image_url`
- `twitter:card` — `summary_large_image` when an image is set, `summary` otherwise
- `og:site_name` — `site_title`

Pages without an image still produce valid tags — they just drop the `og:image` / `twitter:image` lines and downgrade the card type to `summary`.

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
