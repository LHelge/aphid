---
title: About
order: 1
---

**aphid** is a static site generator built for people who want a clear separation between a blog and a reference wiki, without the complexity of a full CMS.

# Origin

The project started as a solution to a personal problem. When my brother and I began converting a classic VW Beetle into an electric car, I set up a blog at [Aphid EV](https://bladlus.se) to track the project and store our research. I started with a [Jekyll](https://jekyllrb.com) site on GitHub Pages, but it quickly became clear that two very different types of content were fighting over the same form:

- **Progress posts** — build log entries documenting what we did, what broke, and what we learned.
- **Reference pages** — research notes on components, wiring diagrams, reverse-engineered protocols — the kind of material you want to read *across*, not *through*.

Blog posts fit Jekyll well. The reference material had to be shoehorned into post form, when what I really wanted was a wiki. I looked at [Hugo](https://gohugo.io) and [Astro](https://astro.build) but couldn't find something that felt right in terms of both features and design. So I built aphid instead.

# Design goals

Three content types, cleanly separated:

- **Blog posts** — date-sorted, tagged, with author metadata. Live at `/blog/<slug>/`.
- **Wiki pages** — reference material with a flat slug-based namespace, auto-generated TOC, and backlinks. Live at `/wiki/<stem>/`.
- **Standalone pages** — About, Contact, and similar nav-level pages. Live at `/<stem>/`.

Cross-linking between all three uses `[[wiki-links]]`: write `[[glossary]]` anywhere and aphid resolves it to whichever file has that filename stem, regardless of content type. A broken link is a hard error in build mode, and renders visibly in serve mode so writing can continue uninterrupted.

# Two entry points

`aphid build` renders the site into `dist/` for deployment — the intended workflow is markdown in git, a build step in GitHub Actions, and GitHub Pages for hosting.

`aphid serve` starts a local dev server with file watching and WebSocket-driven live reload for local writing.

Both modes share the same rendering pipeline; they differ only in how they handle the output and broken wiki-links.

# License

aphid is dual licensed under the [MIT license](https://github.com/LHelge/aphid/blob/main/LICENSE-MIT) and the [Apache License 2.0](https://github.com/LHelge/aphid/blob/main/LICENSE-APACHE), the same way as the Rust language itself. You may use it under either license at your option.

# Source

The source is on [GitHub](https://github.com/LHelge/aphid) and on [crates.io](https://crates.io/crates/aphid). See the [[configuration]] reference to get started, or read the [[wiki-links]] page for the cross-linking syntax.
