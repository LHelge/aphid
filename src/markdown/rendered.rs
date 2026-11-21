use crate::content::{
    BlogFrontmatter, HomePage, NotFoundPage, Page, PageFrontmatter, Site, Slug, WikiFrontmatter,
    WikiIntroPage,
};

use super::{InlineSvgProblem, Rendered};

/// The site with every page body run through the markdown pipeline, joined
/// to the page it came from.
///
/// Produced by [`MarkdownRenderer::render_site`](super::MarkdownRenderer::render_site)
/// in pass 1 of the rendering pipeline. Pass 2 (template rendering) consumes
/// it via the per-kind iterator accessors, which yield `(page, &Rendered)`
/// pairs — no separate slug-keyed lookup, so the "every non-draft page is
/// rendered" invariant is enforced by construction.
///
/// Borrows the [`Site`] it was rendered against; cannot outlive it.
pub struct RenderedSite<'a> {
    site: &'a Site,
    blog: Vec<(&'a Page<BlogFrontmatter>, Rendered)>,
    wiki: Vec<(&'a Page<WikiFrontmatter>, Rendered)>,
    pages: Vec<(&'a Page<PageFrontmatter>, Rendered)>,
    home: Option<(&'a HomePage, Rendered)>,
    not_found: Option<(&'a NotFoundPage, Rendered)>,
    wiki_intro: Option<(&'a WikiIntroPage, Rendered)>,
    diagnostics: Diagnostics,
}

impl<'a> RenderedSite<'a> {
    /// Construct from the per-kind pair lists. `MarkdownRenderer::render_site`
    /// is the only intended caller; tests can use this directly to assemble
    /// a fixture without driving the full markdown pipeline.
    pub(crate) fn from_parts(
        site: &'a Site,
        blog: Vec<(&'a Page<BlogFrontmatter>, Rendered)>,
        wiki: Vec<(&'a Page<WikiFrontmatter>, Rendered)>,
        pages: Vec<(&'a Page<PageFrontmatter>, Rendered)>,
        home: Option<(&'a HomePage, Rendered)>,
        not_found: Option<(&'a NotFoundPage, Rendered)>,
        wiki_intro: Option<(&'a WikiIntroPage, Rendered)>,
    ) -> Self {
        let diagnostics = Diagnostics::collect(
            &blog,
            &wiki,
            &pages,
            home.as_ref(),
            not_found.as_ref(),
            wiki_intro.as_ref(),
        );
        Self {
            site,
            blog,
            wiki,
            pages,
            home,
            not_found,
            wiki_intro,
            diagnostics,
        }
    }

    pub fn site(&self) -> &'a Site {
        self.site
    }

    pub fn blog(&self) -> impl Iterator<Item = (&Page<BlogFrontmatter>, &Rendered)> {
        self.blog.iter().map(|(p, r)| (*p, r))
    }

    pub fn wiki(&self) -> impl Iterator<Item = (&Page<WikiFrontmatter>, &Rendered)> {
        self.wiki.iter().map(|(p, r)| (*p, r))
    }

    pub fn pages(&self) -> impl Iterator<Item = (&Page<PageFrontmatter>, &Rendered)> {
        self.pages.iter().map(|(p, r)| (*p, r))
    }

    pub fn home(&self) -> Option<(&HomePage, &Rendered)> {
        self.home.as_ref().map(|(h, r)| (*h, r))
    }

    pub fn not_found(&self) -> Option<(&NotFoundPage, &Rendered)> {
        self.not_found.as_ref().map(|(n, r)| (*n, r))
    }

    pub fn wiki_intro(&self) -> Option<(&WikiIntroPage, &Rendered)> {
        self.wiki_intro.as_ref().map(|(w, r)| (*w, r))
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
}

/// Build-time signals collected during pass 1: broken wiki-links and
/// images marked for SVG inlining that couldn't be inlined. Future passes
/// (orphan pages, etc.) can extend this without changing the renderer's
/// interface.
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    pub broken_wiki_links: Vec<BrokenWikiLink>,
    pub broken_inline_svgs: Vec<BrokenInlineSvg>,
}

impl Diagnostics {
    pub fn is_empty(&self) -> bool {
        self.broken_wiki_links.is_empty() && self.broken_inline_svgs.is_empty()
    }

    /// The `build` policy: any diagnostic is fatal. Returns
    /// [`Error::Diagnostics`](crate::Error::Diagnostics) listing every
    /// problem, so one run reports them all.
    pub fn ensure_clean(&self) -> Result<(), crate::Error> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(crate::Error::Diagnostics(self.clone()))
        }
    }

    fn collect(
        blog: &[(&Page<BlogFrontmatter>, Rendered)],
        wiki: &[(&Page<WikiFrontmatter>, Rendered)],
        pages: &[(&Page<PageFrontmatter>, Rendered)],
        home: Option<&(&HomePage, Rendered)>,
        not_found: Option<&(&NotFoundPage, Rendered)>,
        wiki_intro: Option<&(&WikiIntroPage, Rendered)>,
    ) -> Self {
        let mut broken_wiki_links = Vec::new();
        let mut broken_inline_svgs = Vec::new();
        let mut record = |source: DiagnosticSource, rendered: &Rendered| {
            for target in &rendered.broken_wiki_links {
                broken_wiki_links.push(BrokenWikiLink {
                    source: source.clone(),
                    target: target.clone(),
                });
            }
            for error in &rendered.inline_svg_errors {
                broken_inline_svgs.push(BrokenInlineSvg {
                    source: source.clone(),
                    src: error.src.clone(),
                    problem: error.problem.clone(),
                });
            }
        };

        let from_pages = blog
            .iter()
            .map(|(p, r)| (DiagnosticSource::Page(p.slug.clone()), r))
            .chain(
                wiki.iter()
                    .map(|(p, r)| (DiagnosticSource::Page(p.slug.clone()), r)),
            )
            .chain(
                pages
                    .iter()
                    .map(|(p, r)| (DiagnosticSource::Page(p.slug.clone()), r)),
            );

        for (source, rendered) in from_pages {
            record(source, rendered);
        }

        for (source, rendered) in [
            home.map(|(_, r)| (DiagnosticSource::Home, r)),
            not_found.map(|(_, r)| (DiagnosticSource::NotFound, r)),
            wiki_intro.map(|(_, r)| (DiagnosticSource::WikiIntro, r)),
        ]
        .into_iter()
        .flatten()
        {
            record(source, rendered);
        }

        Self {
            broken_wiki_links,
            broken_inline_svgs,
        }
    }
}

impl std::fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sections = Vec::new();
        if !self.broken_wiki_links.is_empty() {
            sections.push("broken wiki-links found:".to_owned());
            for link in &self.broken_wiki_links {
                sections.push(format!(
                    "  - page \"{}\" references missing wiki-link: \"{}\"",
                    link.source, link.target
                ));
            }
        }
        if !self.broken_inline_svgs.is_empty() {
            sections.push("SVG images could not be inlined:".to_owned());
            for svg in &self.broken_inline_svgs {
                sections.push(format!(
                    "  - page \"{}\" inlines \"{}\": {}",
                    svg.source, svg.src, svg.problem
                ));
            }
        }
        write!(f, "{}", sections.join("\n"))
    }
}

/// One `#inline` SVG image found during pass 1 that couldn't be inlined.
#[derive(Debug, Clone)]
pub struct BrokenInlineSvg {
    pub source: DiagnosticSource,
    /// The image URL as written in the page.
    pub src: String,
    pub problem: InlineSvgProblem,
}

/// One unresolved `[[wiki-link]]` found during pass 1.
#[derive(Debug, Clone)]
pub struct BrokenWikiLink {
    pub source: DiagnosticSource,
    pub target: String,
}

/// Where a diagnostic originated. `home.md` and `404.md` are special-cased
/// because they have no slug — they're rendered through the same markdown
/// pipeline but aren't [`Page`](crate::content::Page)s.
#[derive(Debug, Clone)]
pub enum DiagnosticSource {
    Page(Slug),
    Home,
    NotFound,
    WikiIntro,
}

impl std::fmt::Display for DiagnosticSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Page(slug) => write!(f, "{slug}"),
            Self::Home => write!(f, "home.md"),
            Self::NotFound => write!(f, "404.md"),
            Self::WikiIntro => write!(f, "wiki.md"),
        }
    }
}
