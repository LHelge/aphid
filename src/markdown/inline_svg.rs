//! Build-time inlining of SVG images.
//!
//! A markdown image whose destination is a local `.svg` with the `#inline`
//! fragment — `![Alt](/static/drawing.svg#inline "Caption")` — is replaced
//! by the file's `<svg>` markup, so the host page's CSS can style it. Plain
//! CommonMark viewers (GitHub, editors) ignore the fragment and show the
//! file as a normal image, so the marker degrades gracefully.
//!
//! The transform:
//!
//! - resolves the path exactly like the `/static/` URL it names (user
//!   `static_dir` first, then the theme's `static/`), refusing `..`
//!   segments, symlinks that leave the static root, and remote URLs;
//! - strips the XML prolog, doctype, comments and processing
//!   instructions, keeping the root `<svg>` attributes as they are;
//! - gives the SVG an accessible name from the alt text (`role="img"` +
//!   `aria-label`), or `aria-hidden="true"` when the alt text is empty;
//! - prefixes every `id` with a per-instance prefix and rewrites the
//!   references to it (`url(#…)`, `href="#…"`, `xlink:href="#…"`,
//!   `aria-labelledby` / `aria-describedby`), so several inlined SVGs on
//!   one page never collide.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use percent_encoding::percent_decode_str;
use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use regex::Regex;

use crate::config::Config;
use crate::html::escape_html;

/// URL fragment that opts an SVG image into inlining.
pub const INLINE_MARKER: &str = "#inline";

/// CSS class on the `<figure>` / `<span>` wrapping an inlined SVG.
pub const INLINE_SVG_CLASS: &str = "inline-svg";

/// Site-root URL prefix that maps onto the static directories.
const STATIC_URL_PREFIX: &str = "/static/";

/// `url(#id)`, with optional quotes and whitespace, as used by `fill`,
/// `clip-path`, `marker-end`, `style` and CSS inside `<style>`.
static URL_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"url\(\s*(['"]?)#([^'")\s]+)(['"]?)\s*\)"#).unwrap());

/// Why an image marked for inlining could not be inlined.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InlineSvgProblem {
    #[error("remote URLs are never fetched; only local files can be inlined")]
    Remote,
    #[error("only .svg files can be inlined")]
    NotSvg,
    #[error("only files under /static/ can be inlined")]
    NotStatic,
    #[error("path leaves the static directory")]
    OutsideSite,
    #[error("file not found")]
    NotFound,
    #[error("could not read file: {0}")]
    Read(String),
    #[error("not valid SVG: {0}")]
    Invalid(String),
}

/// One image that was marked `#inline` but could not be inlined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSvgError {
    /// The image destination as written (after root-relative rewriting).
    pub src: String,
    pub problem: InlineSvgProblem,
}

/// Replaces `#inline`-marked SVG images with their markup. Built once per
/// [`MarkdownRenderer`](super::MarkdownRenderer) from the site config.
pub struct SvgInliner {
    /// Canonicalized static roots, in lookup order (user, then theme).
    roots: Vec<PathBuf>,
}

impl SvgInliner {
    pub fn new(config: &Config) -> Self {
        let candidates = [
            Some(config.static_dir.clone()),
            config.theme_dir.as_ref().map(|d| d.join("static")),
        ];
        let roots = candidates
            .into_iter()
            .flatten()
            .filter_map(|dir| dir.canonicalize().ok())
            .filter(|dir| dir.is_dir())
            .collect();
        Self { roots }
    }

    /// Rewrite the event stream. `seed` (the page body) makes the id prefix
    /// unique per page, so two pages shown together (e.g. an index that
    /// embeds post bodies) never collide either.
    ///
    /// An image alone in its paragraph becomes a `<figure>` with an
    /// optional `<figcaption>` (from the image title). An image inside
    /// running text becomes a `<span>`, since a `<figure>` cannot live
    /// inside a `<p>`; it has no caption.
    ///
    /// Images that fail to inline are replaced by a
    /// `<span class="inline-svg broken">` holding the alt text and are
    /// returned as errors: `build` fails on them, `serve` warns.
    pub fn transform<'e>(
        &self,
        events: Vec<Event<'e>>,
        seed: &str,
    ) -> (Vec<Event<'e>>, Vec<InlineSvgError>) {
        let seed_hash = fnv1a(seed.as_bytes());
        let mut out: Vec<Event<'e>> = Vec::with_capacity(events.len());
        let mut errors = Vec::new();
        let mut instance = 0usize;
        let mut iter = events.into_iter().peekable();

        while let Some(event) = iter.next() {
            let Event::Start(Tag::Image {
                dest_url, title, ..
            }) = &event
            else {
                out.push(event);
                continue;
            };
            let Some(src) = dest_url.strip_suffix(INLINE_MARKER) else {
                out.push(event);
                continue;
            };
            let src = src.to_owned();
            let title = title.to_string();

            // Collect the alt text up to the matching End(Image), the same
            // way pulldown-cmark's HTML writer flattens it.
            let mut alt = String::new();
            for inner in iter.by_ref() {
                match inner {
                    Event::End(TagEnd::Image) => break,
                    Event::Text(t) | Event::Code(t) | Event::InlineMath(t) => alt.push_str(&t),
                    Event::SoftBreak | Event::HardBreak => alt.push(' '),
                    _ => {}
                }
            }

            instance += 1;
            let prefix = format!("svg{seed_hash:08x}-{instance}-");
            let svg = self
                .load(&src)
                .and_then(|text| InlineSvg::parse(&text, &alt, &prefix));

            let svg = match svg {
                Ok(svg) => svg,
                Err(problem) => {
                    out.push(Event::InlineHtml(CowStr::from(format!(
                        "<span class=\"{INLINE_SVG_CLASS} broken\" title=\"{}\">{}</span>",
                        escape_html(&problem.to_string()),
                        escape_html(&alt)
                    ))));
                    errors.push(InlineSvgError { src, problem });
                    continue;
                }
            };

            let standalone = matches!(out.last(), Some(Event::Start(Tag::Paragraph)))
                && matches!(iter.peek(), Some(Event::End(TagEnd::Paragraph)));
            if standalone {
                out.pop();
                iter.next();
                let caption = if title.is_empty() {
                    String::new()
                } else {
                    format!("<figcaption>{}</figcaption>", escape_html(&title))
                };
                out.push(Event::Html(CowStr::from(format!(
                    "<figure class=\"{INLINE_SVG_CLASS}\">{}{caption}</figure>\n",
                    svg.markup
                ))));
            } else {
                out.push(Event::InlineHtml(CowStr::from(format!(
                    "<span class=\"{INLINE_SVG_CLASS}\">{}</span>",
                    svg.markup
                ))));
            }
        }

        (out, errors)
    }

    /// Read the file behind a root-relative image URL.
    fn load(&self, src: &str) -> Result<String, InlineSvgProblem> {
        let path = self.resolve(src)?;
        let bytes = std::fs::read(&path).map_err(|e| InlineSvgProblem::Read(e.to_string()))?;
        String::from_utf8(bytes).map_err(|_| InlineSvgProblem::Invalid("not UTF-8".into()))
    }

    /// Map a `/static/…` URL onto a file inside one of the static roots.
    /// Rejects anything that could reach outside them.
    fn resolve(&self, src: &str) -> Result<PathBuf, InlineSvgProblem> {
        if src.starts_with("//") || src.contains("://") {
            return Err(InlineSvgProblem::Remote);
        }
        let path = src.split(['?', '#']).next().unwrap_or_default();
        if !path.to_ascii_lowercase().ends_with(".svg") {
            return Err(InlineSvgProblem::NotSvg);
        }
        let Some(rel) = path.strip_prefix(STATIC_URL_PREFIX) else {
            return Err(InlineSvgProblem::NotStatic);
        };

        // Decode per segment so an encoded `%2F` can't smuggle in a
        // separator, and join segment by segment so nothing can turn the
        // relative path into an absolute one.
        let mut relative = PathBuf::new();
        for segment in rel.split('/') {
            let segment = percent_decode_str(segment)
                .decode_utf8()
                .map_err(|_| InlineSvgProblem::Invalid("path is not UTF-8".into()))?;
            match segment.as_ref() {
                "" | "." => {}
                ".." => return Err(InlineSvgProblem::OutsideSite),
                s if s.contains(['/', '\\', '\0']) => {
                    return Err(InlineSvgProblem::OutsideSite);
                }
                s => relative.push(s),
            }
        }

        for root in &self.roots {
            let candidate = root.join(&relative);
            if candidate.symlink_metadata().is_err() {
                continue;
            }
            return Self::contained(root, &candidate);
        }
        Err(InlineSvgProblem::NotFound)
    }

    /// Canonicalize `candidate` (following symlinks) and check it is still
    /// a regular file under `root`.
    fn contained(root: &Path, candidate: &Path) -> Result<PathBuf, InlineSvgProblem> {
        let canonical = candidate
            .canonicalize()
            .map_err(|_| InlineSvgProblem::NotFound)?;
        if !canonical.starts_with(root) {
            return Err(InlineSvgProblem::OutsideSite);
        }
        if !canonical.is_file() {
            return Err(InlineSvgProblem::NotFound);
        }
        Ok(canonical)
    }
}

/// One SVG document, rewritten for inlining into an HTML page.
#[derive(Debug)]
struct InlineSvg {
    markup: String,
}

impl InlineSvg {
    /// Validate `text` as an SVG document and re-serialize its root element
    /// with prolog, doctype, comments and PIs dropped, the accessible name
    /// set from `alt`, and every id prefixed with `prefix`.
    fn parse(text: &str, alt: &str, prefix: &str) -> Result<Self, InlineSvgProblem> {
        let ids = Self::collect_ids(text)?;
        let rewriter = IdRewriter { ids, prefix };

        let mut reader = Reader::from_str(text);
        let mut markup = String::with_capacity(text.len() + 64);
        let mut depth = 0usize;
        let mut seen_root = false;
        let mut in_style = 0usize;

        loop {
            let event = reader.read_event().map_err(Self::xml_error)?;
            match event {
                XmlEvent::Start(e) | XmlEvent::Empty(e) if depth == 0 && seen_root => {
                    return Err(Self::invalid(format!(
                        "second top-level element <{}>",
                        e.name().as_ref()
                    )));
                }
                XmlEvent::Start(e) => {
                    let root = Self::check_root(&e, depth, &mut seen_root)?;
                    if e.local_name().as_ref() == "style" {
                        in_style += 1;
                    }
                    rewriter.write_tag(&mut markup, &e, root.then_some(alt), false)?;
                    depth += 1;
                }
                XmlEvent::Empty(e) => {
                    let root = Self::check_root(&e, depth, &mut seen_root)?;
                    rewriter.write_tag(&mut markup, &e, root.then_some(alt), true)?;
                }
                XmlEvent::End(e) => {
                    depth = depth.saturating_sub(1);
                    if e.local_name().as_ref() == "style" {
                        in_style = in_style.saturating_sub(1);
                    }
                    markup.push_str("</");
                    markup.push_str(e.name().as_ref());
                    markup.push('>');
                }
                XmlEvent::Text(t) => {
                    let raw = t.into_inner();
                    if depth == 0 {
                        if !raw.trim().is_empty() {
                            return Err(Self::invalid("text outside the root element"));
                        }
                    } else if in_style > 0 {
                        markup.push_str(&rewriter.rewrite_url_refs(&raw));
                    } else {
                        markup.push_str(&raw);
                    }
                }
                XmlEvent::CData(c) if depth > 0 => {
                    let raw = c.into_inner();
                    markup.push_str("<![CDATA[");
                    if in_style > 0 {
                        markup.push_str(&rewriter.rewrite_url_refs(&raw));
                    } else {
                        markup.push_str(&raw);
                    }
                    markup.push_str("]]>");
                }
                XmlEvent::CData(_) => {
                    return Err(Self::invalid("CDATA outside the root element"));
                }
                XmlEvent::GeneralRef(r) => {
                    if depth == 0 {
                        return Err(Self::invalid("text outside the root element"));
                    }
                    markup.push('&');
                    markup.push_str(&r.into_inner());
                    markup.push(';');
                }
                XmlEvent::Comment(_) | XmlEvent::Decl(_) | XmlEvent::PI(_) => {}
                XmlEvent::DocType(_) => {}
                XmlEvent::Eof => break,
            }
        }

        if !seen_root {
            return Err(Self::invalid("no <svg> root element"));
        }
        if depth != 0 {
            return Err(Self::invalid("unclosed element"));
        }
        Ok(Self { markup })
    }

    /// First pass: every `id` attribute value in the document.
    fn collect_ids(text: &str) -> Result<HashSet<String>, InlineSvgProblem> {
        let mut reader = Reader::from_str(text);
        let mut ids = HashSet::new();
        loop {
            match reader.read_event().map_err(Self::xml_error)? {
                XmlEvent::Start(e) | XmlEvent::Empty(e) => {
                    for attr in e.attributes() {
                        let attr = attr.map_err(|e| Self::invalid(e.to_string()))?;
                        if attr.key.as_ref() == "id" {
                            ids.insert(attr.value.into_owned());
                        }
                    }
                }
                XmlEvent::Eof => break,
                _ => {}
            }
        }
        Ok(ids)
    }

    /// Mark the root as seen; error if the root element isn't `<svg>`.
    /// Returns whether `e` is the root.
    fn check_root(
        e: &BytesStart<'_>,
        depth: usize,
        seen_root: &mut bool,
    ) -> Result<bool, InlineSvgProblem> {
        if depth > 0 {
            return Ok(false);
        }
        if e.name().as_ref() != "svg" {
            return Err(Self::invalid(format!(
                "root element is <{}>, expected <svg>",
                e.name().as_ref()
            )));
        }
        *seen_root = true;
        Ok(true)
    }

    fn xml_error(e: quick_xml::Error) -> InlineSvgProblem {
        Self::invalid(e.to_string())
    }

    fn invalid(message: impl Into<String>) -> InlineSvgProblem {
        InlineSvgProblem::Invalid(message.into())
    }
}

/// Rewrites ids and id references with a per-instance prefix. Only
/// references to ids defined in the same document are touched, so links
/// to other documents and fragments of external files stay intact.
struct IdRewriter<'a> {
    ids: HashSet<String>,
    prefix: &'a str,
}

impl IdRewriter<'_> {
    /// Serialize a start / empty tag. `alt` is `Some` for the root `<svg>`,
    /// whose accessibility attributes are replaced.
    fn write_tag(
        &self,
        out: &mut String,
        e: &BytesStart<'_>,
        alt: Option<&str>,
        empty: bool,
    ) -> Result<(), InlineSvgProblem> {
        out.push('<');
        out.push_str(e.name().as_ref());
        for attr in e.attributes() {
            let attr = attr.map_err(|e| InlineSvg::invalid(e.to_string()))?;
            let key = attr.key.as_ref();
            if alt.is_some()
                && matches!(
                    key,
                    "role" | "aria-label" | "aria-labelledby" | "aria-hidden"
                )
            {
                continue;
            }
            let value = self.rewrite_attr(key, &attr.value);
            out.push(' ');
            out.push_str(key);
            out.push_str("=\"");
            // The raw value may have been single-quoted in the source.
            out.push_str(&value.replace('"', "&quot;"));
            out.push('"');
        }
        if let Some(alt) = alt {
            if alt.is_empty() {
                out.push_str(" aria-hidden=\"true\"");
            } else {
                out.push_str(" role=\"img\" aria-label=\"");
                out.push_str(&escape_html(alt));
                out.push('"');
            }
        }
        out.push_str(if empty { "/>" } else { ">" });
        Ok(())
    }

    fn rewrite_attr(&self, key: &str, value: &str) -> String {
        let local = key.rsplit(':').next().unwrap_or(key);
        match local {
            "id" if key == "id" => format!("{}{value}", self.prefix),
            "href" => match value.strip_prefix('#') {
                Some(id) if self.ids.contains(id) => format!("#{}{id}", self.prefix),
                _ => value.to_owned(),
            },
            "aria-labelledby" | "aria-describedby" => value
                .split_whitespace()
                .map(|id| {
                    if self.ids.contains(id) {
                        format!("{}{id}", self.prefix)
                    } else {
                        id.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => self.rewrite_url_refs(value),
        }
    }

    fn rewrite_url_refs(&self, text: &str) -> String {
        if !text.contains("url(") {
            return text.to_owned();
        }
        URL_REF_RE
            .replace_all(text, |caps: &regex::Captures| {
                let id = &caps[2];
                if self.ids.contains(id) {
                    format!("url({}#{}{id}{})", &caps[1], self.prefix, &caps[3])
                } else {
                    caps[0].to_owned()
                }
            })
            .into_owned()
    }
}

/// 32-bit FNV-1a. Stable across Rust releases (unlike `DefaultHasher`), so
/// the id prefixes, and with them the build output, are reproducible.
fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |hash, &b| {
        (hash ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::write_file;
    use pulldown_cmark::{Options, Parser};
    use tempfile::TempDir;

    const DRAWING: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<!-- generated by a tool -->
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="wirebug" viewBox="0 0 10 10">
  <defs><marker id="arrow"><path d="M0 0L1 1"/></marker></defs>
  <style>.w { marker-end: url(#arrow); }</style>
  <line class="w" data-color="red" stroke="red" marker-end="url(#arrow)" x1="0" y1="0" x2="5" y2="5"/>
  <use xlink:href="#arrow" href="#arrow"/>
  <a href="#elsewhere">x</a>
</svg>
"##;

    struct Fixture {
        dir: TempDir,
        inliner: SvgInliner,
    }

    impl Fixture {
        fn new(files: &[(&str, &str)]) -> Self {
            let dir = TempDir::new().unwrap();
            std::fs::create_dir_all(dir.path().join("static")).unwrap();
            for (path, content) in files {
                write_file(&dir.path().join("static").join(path), content);
            }
            let config: Config = format!(
                "title = \"T\"\nbase_url = \"http://x\"\nstatic_dir = \"{}\"",
                dir.path().join("static").display()
            )
            .parse()
            .unwrap();
            let inliner = SvgInliner::new(&config);
            Self { dir, inliner }
        }

        fn render(&self, md: &str) -> (String, Vec<InlineSvgError>) {
            let events: Vec<_> = Parser::new_ext(md, Options::empty()).collect();
            let events = crate::markdown::rewrite_relative_urls(events);
            let (events, errors) = self.inliner.transform(events, md);
            (crate::markdown::render_html(events), errors)
        }
    }

    #[test]
    fn unmarked_svg_stays_img() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, errors) = fx.render("![Logo](/static/d.svg)\n");
        assert!(
            html.contains(r#"<img src="/static/d.svg" alt="Logo" />"#),
            "{html}"
        );
        assert!(!html.contains("<svg"));
        assert!(errors.is_empty());
    }

    #[test]
    fn marked_svg_inlined_as_figure_with_caption() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, errors) = fx.render("![Wiring diagram](/static/d.svg#inline \"Figure 1\")\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert!(
            html.starts_with(r#"<figure class="inline-svg"><svg "#),
            "{html}"
        );
        assert!(
            !html.contains("<p>"),
            "figure must not sit in a <p>: {html}"
        );
        assert!(html.contains(r#"role="img" aria-label="Wiring diagram""#));
        assert!(html.contains("<figcaption>Figure 1</figcaption></figure>"));
        assert!(!html.contains("<img"));
    }

    #[test]
    fn prolog_doctype_and_comments_stripped_root_attrs_kept() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, _) = fx.render("![A](/static/d.svg#inline)\n");
        assert!(!html.contains("<?xml"));
        assert!(!html.contains("DOCTYPE"));
        assert!(!html.contains("generated by"));
        assert!(html.contains(r#"class="wirebug""#));
        assert!(html.contains(r#"viewBox="0 0 10 10""#));
        assert!(html.contains(r#"data-color="red""#));
        assert!(!html.contains("<figcaption>"), "no title, no caption");
    }

    #[test]
    fn relative_path_resolved_against_site_root() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, errors) = fx.render("![A](static/d.svg#inline)\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert!(html.contains("<svg"));
    }

    #[test]
    fn ids_and_references_prefixed() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, _) = fx.render("![A](/static/d.svg#inline)\n");
        let p = Regex::new(r#"id="(svg[0-9a-f]{8}-1-)arrow""#).unwrap();
        let prefix = &p.captures(&html).expect("prefixed id")[1];
        assert!(html.contains(&format!(r#"marker-end="url(#{prefix}arrow)""#)));
        assert!(html.contains(&format!("marker-end: url(#{prefix}arrow)")));
        assert!(html.contains(&format!(r##"xlink:href="#{prefix}arrow""##)));
        assert!(html.contains(&format!(r##" href="#{prefix}arrow""##)));
        // A fragment that isn't an id in this document is left alone.
        assert!(html.contains(r##"href="#elsewhere""##));
    }

    #[test]
    fn two_svgs_on_one_page_get_distinct_prefixes() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, errors) =
            fx.render("![A](/static/d.svg#inline)\n\n![B](/static/d.svg#inline)\n");
        assert!(errors.is_empty());
        let ids: Vec<_> = Regex::new(r#"id="([^"]+)""#)
            .unwrap()
            .captures_iter(&html)
            .map(|c| c[1].to_owned())
            .collect();
        assert_eq!(ids.len(), 2, "{html}");
        assert_ne!(ids[0], ids[1]);
        assert_eq!(html.matches("<figure").count(), 2);
    }

    #[test]
    fn same_svg_on_different_pages_gets_distinct_prefixes() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (a, _) = fx.render("Page A\n\n![A](/static/d.svg#inline)\n");
        let (b, _) = fx.render("Page B\n\n![A](/static/d.svg#inline)\n");
        let re = Regex::new(r#"id="([^"]+)""#).unwrap();
        assert_ne!(&re.captures(&a).unwrap()[1], &re.captures(&b).unwrap()[1]);
    }

    #[test]
    fn empty_alt_marks_svg_decorative() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, _) = fx.render("![](/static/d.svg#inline)\n");
        assert!(html.contains(r#"aria-hidden="true""#));
        assert!(!html.contains("role=\"img\""));
    }

    #[test]
    fn alt_text_escaped_and_existing_aria_replaced() {
        let svg = r#"<svg role="presentation" aria-label="old" viewBox="0 0 1 1"/>"#;
        let fx = Fixture::new(&[("d.svg", svg)]);
        let (html, _) = fx.render("![Tom & \"Jerry\"](/static/d.svg#inline)\n");
        assert!(
            html.contains(r#"aria-label="Tom &amp; &quot;Jerry&quot;""#),
            "{html}"
        );
        assert!(!html.contains("presentation"));
        assert!(!html.contains("old"));
    }

    #[test]
    fn image_in_running_text_becomes_span() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, _) = fx.render("Before ![A](/static/d.svg#inline \"cap\") after.\n");
        assert!(
            html.starts_with("<p>Before <span class=\"inline-svg\"><svg"),
            "{html}"
        );
        assert!(html.contains("</svg></span> after.</p>"));
        assert!(!html.contains("<figure"));
    }

    #[test]
    fn missing_file_is_error() {
        let fx = Fixture::new(&[]);
        let (html, errors) = fx.render("![A](/static/missing.svg#inline)\n");
        assert_eq!(
            errors,
            vec![InlineSvgError {
                src: "/static/missing.svg".into(),
                problem: InlineSvgProblem::NotFound
            }]
        );
        assert!(html.contains(r#"class="inline-svg broken""#));
    }

    #[test]
    fn invalid_files_are_errors() {
        let fx = Fixture::new(&[
            ("broken.svg", "<svg><g></svg>"),
            ("html.svg", "<html></html>"),
            ("empty.svg", ""),
            ("two.svg", "<svg/><svg/>"),
        ]);
        for name in ["broken", "html", "empty", "two"] {
            let (_, errors) = fx.render(&format!("![A](/static/{name}.svg#inline)\n"));
            assert!(
                matches!(
                    errors.as_slice(),
                    [InlineSvgError {
                        problem: InlineSvgProblem::Invalid(_),
                        ..
                    }]
                ),
                "{name}: {errors:?}"
            );
        }
    }

    #[test]
    fn traversal_rejected() {
        let fx = Fixture::new(&[]);
        write_file(&fx.dir.path().join("secret.svg"), "<svg/>");
        for src in [
            "/static/../secret.svg",
            "/static/%2e%2e/secret.svg",
            "/static/..%2Fsecret.svg",
        ] {
            let (_, errors) = fx.render(&format!("![A]({src}#inline)\n"));
            assert_eq!(errors[0].problem, InlineSvgProblem::OutsideSite, "{src}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlink_out_of_static_rejected() {
        let fx = Fixture::new(&[]);
        write_file(&fx.dir.path().join("secret.svg"), "<svg/>");
        std::os::unix::fs::symlink(
            fx.dir.path().join("secret.svg"),
            fx.dir.path().join("static/link.svg"),
        )
        .unwrap();
        let (_, errors) = fx.render("![A](/static/link.svg#inline)\n");
        assert_eq!(errors[0].problem, InlineSvgProblem::OutsideSite);
    }

    #[test]
    fn remote_non_static_and_non_svg_rejected() {
        let fx = Fixture::new(&[("x.png", "png")]);
        let cases = [
            ("https://example.com/x.svg", InlineSvgProblem::Remote),
            ("//example.com/x.svg", InlineSvgProblem::Remote),
            ("/other/x.svg", InlineSvgProblem::NotStatic),
            ("/static/x.png", InlineSvgProblem::NotSvg),
        ];
        for (src, expected) in cases {
            let (_, errors) = fx.render(&format!("![A]({src}#inline)\n"));
            assert_eq!(errors[0].problem, expected, "{src}");
        }
    }

    #[test]
    fn other_fragments_not_inlined() {
        let fx = Fixture::new(&[("d.svg", DRAWING)]);
        let (html, errors) = fx.render("![A](/static/d.svg#view)\n");
        assert!(html.contains("<img"));
        assert!(errors.is_empty());
    }
}
