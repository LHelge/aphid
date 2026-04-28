# Built for speed

**aphid** is written in Rust which is blazingly fast and allows fearless concurrency. As a result rendering your site is multithreaded to utilize all cores of your CPU and be *really* quick.

The [[dev server]] rebuilds run on the rayon threadpool while tokio keeps serving HTTP and WebSocket traffic. File-change reloads land before you've switched windows.

---

# Simple project structure

Keep content and configuration in one clean directory. aphid watches for changes and rebuilds only what's needed. The same Markdown drives the blog, the wiki, and standalone pages — and `[[wiki-links]]` resolve across all three.

```
my-site/
├── aphid.toml
├── content/
│   ├── home.md
│   ├── blog/
│   │   └── first-post.md
│   ├── wiki/
│   │   └── installation.md
│   └── pages/
│       └── about.md
└── static/
    └── images/
```

---

# Flexible theming

Themes are plain [Tera](https://keats.github.io/tera/) templates plus static assets — swap the entire look with a single `theme` line in `aphid.toml`, or fork the default and tweak. Loops, conditionals, and inheritance give you full control over the rendered HTML.

---

[Read the docs →](/wiki/installation/)

