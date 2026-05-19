use crate::content::{
    BlogFrontmatter, HomePage, NotFoundPage, Page, PageFrontmatter, Site, Slug, WikiFrontmatter,
    WikiIntroPage,
};

use super::Rendered;

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

/// Build-time signals collected during pass 1. Today this is only broken
/// wiki-links; future passes (orphan pages, missing image refs, etc.) can
/// extend this without changing the renderer's interface.
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    pub broken_wiki_links: Vec<BrokenWikiLink>,
}

impl Diagnostics {
    pub fn is_empty(&self) -> bool {
        self.broken_wiki_links.is_empty()
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
            for target in &rendered.broken_wiki_links {
                broken_wiki_links.push(BrokenWikiLink {
                    source: source.clone(),
                    target: target.clone(),
                });
            }
        }

        for (source, rendered) in [
            home.map(|(_, r)| (DiagnosticSource::Home, r)),
            not_found.map(|(_, r)| (DiagnosticSource::NotFound, r)),
            wiki_intro.map(|(_, r)| (DiagnosticSource::WikiIntro, r)),
        ]
        .into_iter()
        .flatten()
        {
            for target in &rendered.broken_wiki_links {
                broken_wiki_links.push(BrokenWikiLink {
                    source: source.clone(),
                    target: target.clone(),
                });
            }
        }

        Self { broken_wiki_links }
    }
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
