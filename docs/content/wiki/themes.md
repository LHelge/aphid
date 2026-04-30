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

## blog_post.html

| Variable | Type | Description |
|----------|------|-------------|
| `title` | string | Post title from frontmatter |
| `url` | string | Clean URL, e.g. `/blog/my-post/` |
| `content` | string | Rendered HTML body |
| `toc` | list | Heading entries; each has `level`, `text`, and `id` |
| `backlinks` | list | Pages that link here; each has `title` and `url` |
| `author` | string? | From frontmatter |
| `image` | string? | Hero/headline image path or URL, from frontmatter |
| `created` | string? | Publication date, formatted `YYYY-MM-DD` |
| `updated` | string? | Last-edited date |
| `tags` | list | Each tag has `name` and `slug` |

## wiki_page.html

Same variables as `blog_post.html`. `author` and `image` are always absent; `created`, `updated`, and `tags` are present only if set in frontmatter. In addition:

| Variable | Type | Description |
|----------|------|-------------|
| `category` | string? | Category for this page, from frontmatter |
| `wiki_categories` | list | All wiki pages grouped by category — for rendering a sidebar with the current page highlighted (compare `page.url == url`). Each entry has `name` (string?) and `pages` (list of `{title, url}`). Named categories come first alphabetically; uncategorised pages are grouped under `name = null` at the end. |

## page.html

Same variables as `blog_post.html`. `author`, `image`, `created`, `updated`, and `tags` are always absent.

## home.html

Renders the site root (`/index.html`). Receives the post list plus an optional rendered `home` block from `content/home.md` (see [[configuration]]).

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list | All blog posts — see the post entry shape below |
| `home` | object? | Present when `content/home.md` exists. Has `content` (string, the rendered HTML — pass through `\| safe`). |

## blog_index.html

Renders the blog listing at `/blog/`.

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list | All blog posts — see the post entry shape below |

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
| `categories` | list | All wiki pages grouped by category. Each entry has `name` (string?) and `pages` (list of `{title, url}`). Named categories come first alphabetically; uncategorised pages are grouped under `name = null` at the end. |

## tag.html

| Variable | Type | Description |
|----------|------|-------------|
| `tag` | string | Tag display name |
| `tag_slug` | string | URL-safe slug |
| `posts` | list | Tagged posts; each has `title`, `url`, and `created?` |

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
