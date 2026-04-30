---
title: Pagination
category: Content
tags:
  - reference
---

`aphid` paginates the blog index and per-tag pages once a listing grows past `posts_per_page`. Page 1 stays at the canonical URL; pages 2 and beyond live under `page/N/`.

# URLs

| Listing | Page 1 | Page N (N > 1) |
|---------|--------|----------------|
| Blog index | `/blog/` | `/blog/page/N/` |
| Tag page | `/tags/{slug}/` | `/tags/{slug}/page/N/` |

This keeps page 1 free of the `page/1/` suffix so the canonical URL never changes when a site is small. The home page is not paginated — it always shows the full posts list as a recent-posts preview.

# Configuration

Set `posts_per_page` in `aphid.toml`:

```toml
posts_per_page = 10
```

Default is `10`. See [[configuration]] for the full list of fields.

# Template variable

Both `blog_index.html` and `tag.html` receive a `pagination` variable. It is `null` when every post fits on one page (so the nav UI stays hidden); otherwise it has:

| Field | Type | Description |
|-------|------|-------------|
| `current` | int | Current page number (1-indexed) |
| `total` | int | Total page count |
| `prev_url` | string? | URL of the previous page, or `null` on page 1 |
| `next_url` | string? | URL of the next page, or `null` on the last page |
| `pages` | list | Every page; each entry has `n` (page number) and `url` |

A minimal `pagination.html` partial:

```jinja
<nav class="pagination" aria-label="Pagination">
  {% if pagination.prev_url %}
  <a rel="prev" href="{{ pagination.prev_url | safe }}">&larr; Newer</a>
  {% endif %}
  <ol>
    {% for link in pagination.pages %}
    <li>
      {% if link.n == pagination.current %}
      <span aria-current="page">{{ link.n }}</span>
      {% else %}
      <a href="{{ link.url | safe }}">{{ link.n }}</a>
      {% endif %}
    </li>
    {% endfor %}
  </ol>
  {% if pagination.next_url %}
  <a rel="next" href="{{ pagination.next_url | safe }}">Older &rarr;</a>
  {% endif %}
</nav>
```

Include it from `blog_index.html` and `tag.html`, gated on the variable being set:

```jinja
{% if pagination %}{% include "pagination.html" %}{% endif %}
```

Gating with `{% if pagination %}` at the include site (rather than inside `pagination.html`) keeps the rendered output clean — when the variable is `null`, nothing about the include leaks through.

# Page-1-only treatment

A theme that highlights the newest post (a "featured" or hero card) should gate that treatment on page 1, since on `/blog/page/2/` the first post in `posts` is no longer the latest:

```jinja
{% if not pagination or pagination.current == 1 %}
  {# featured layout: first post styled prominently #}
{% else %}
  {# uniform layout: every post in the same compact format #}
{% endif %}
```

The bundled docs theme uses exactly this pattern.

See also: [[configuration]], [[themes]].
