use std::collections::HashMap;

use serde::Serialize;

use crate::config::{Config, Social};
use crate::content::page::{Page, PageKind};
use crate::content::slug::Slug;
use crate::content::{PageAny, PageFrontmatter, Site, WikiFrontmatter};
use crate::generated::FaviconSet;
use crate::markdown::{HeadingEntry, Rendered};

fn owned(value: Option<&str>) -> Option<String> {
    value.map(str::to_owned)
}

/// A single nav entry for standalone pages, available to all templates.
#[derive(Debug, Clone, Serialize)]
pub struct NavEntry {
    pub title: String,
    pub url: String,
}

impl From<&Page<PageFrontmatter>> for NavEntry {
    fn from(page: &Page<PageFrontmatter>) -> Self {
        Self {
            title: page.title().to_string(),
            url: PageKind::Page.url_path(&page.slug),
        }
    }
}

impl NavEntry {
    pub fn from_pages(pages: &[Page<PageFrontmatter>]) -> Vec<Self> {
        let mut entries: Vec<_> = pages
            .iter()
            .map(|p| {
                let order = p.frontmatter.order.unwrap_or(i32::MAX);
                (order, NavEntry::from(p))
            })
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.title.cmp(&b.1.title)));
        entries.into_iter().map(|(_, entry)| entry).collect()
    }
}

/// A TOC entry exposed to templates.
#[derive(Debug, Clone, Serialize)]
pub struct TocEntry {
    pub level: u8,
    pub text: String,
    pub id: Slug,
}

impl From<&HeadingEntry> for TocEntry {
    fn from(h: &HeadingEntry) -> Self {
        Self {
            level: h.level,
            text: h.text.clone(),
            id: h.id.clone(),
        }
    }
}

impl TocEntry {
    fn from_headings(headings: &[HeadingEntry]) -> Vec<Self> {
        headings.iter().map(Self::from).collect()
    }
}

/// A backlink entry exposed to templates.
#[derive(Debug, Clone, Serialize)]
pub struct BacklinkEntry {
    pub title: String,
    pub url: String,
}

impl From<&PageAny<'_>> for BacklinkEntry {
    fn from(page: &PageAny<'_>) -> Self {
        Self {
            title: page.title().into_owned(),
            url: page.url_path(),
        }
    }
}

/// A tag reference with both a display name and a URL-safe slug.
#[derive(Debug, Clone, Serialize)]
pub struct TagRef {
    pub name: String,
    pub slug: Slug,
}

impl From<&str> for TagRef {
    fn from(tag: &str) -> Self {
        Self {
            name: tag.to_owned(),
            slug: tag.into(),
        }
    }
}

impl TagRef {
    fn from_tags(tags: &[String]) -> Vec<Self> {
        tags.iter().map(|tag| Self::from(tag.as_str())).collect()
    }
}

/// A blog post summary for index/tag listing pages.
#[derive(Debug, Clone, Serialize)]
pub struct PostEntry {
    pub title: String,
    pub url: String,
    pub created: Option<String>,
    pub image: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<TagRef>,
}

impl PostEntry {
    /// Build a `PostEntry` from any kind of page. Image, description, and
    /// tags are populated only for blog posts; other kinds get `None` /
    /// empty.
    pub fn from_page(any: &PageAny<'_>) -> Self {
        Self {
            title: any.title().into_owned(),
            url: any.url_path(),
            created: any.created(),
            image: owned(any.image()),
            description: owned(any.description()),
            tags: TagRef::from_tags(any.tags()),
        }
    }

    pub fn from_pages<'a>(pages: impl IntoIterator<Item = PageAny<'a>>) -> Vec<Self> {
        pages
            .into_iter()
            .map(|page| Self::from_page(&page))
            .collect()
    }
}

/// A single page link in the paginator's numeric nav.
#[derive(Debug, Clone, Serialize)]
pub struct PageLink {
    pub n: usize,
    pub url: String,
}

/// Pagination state for index-style pages (blog index, tag pages).
///
/// `current` and `total` are 1-indexed. `prev_url` / `next_url` are
/// `None` at the boundaries. `pages` carries every page's URL so
/// templates can render numeric nav.
#[derive(Debug, Clone, Serialize)]
pub struct Pagination {
    pub current: usize,
    pub total: usize,
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
    pub pages: Vec<PageLink>,
}

impl Pagination {
    /// Build pagination state for the page at `current` (1-indexed) of
    /// `total` pages, all under `base_path` (which must end with `/`,
    /// e.g. `/blog/` or `/tags/rust/`).
    ///
    /// Returns `None` when there is only one page — templates use that
    /// to hide the nav UI entirely.
    pub fn build(base_path: &str, current: usize, total: usize) -> Option<Self> {
        if total <= 1 {
            return None;
        }
        let url_for = |n: usize| {
            if n == 1 {
                base_path.to_string()
            } else {
                format!("{base_path}page/{n}/")
            }
        };
        let pages = (1..=total)
            .map(|n| PageLink { n, url: url_for(n) })
            .collect();
        Some(Self {
            current,
            total,
            prev_url: (current > 1).then(|| url_for(current - 1)),
            next_url: (current < total).then(|| url_for(current + 1)),
            pages,
        })
    }
}

/// Context for the blog index page (list of posts on one paginated page).
#[derive(Debug, Serialize)]
pub struct BlogIndexContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub posts: Vec<PostEntry>,
    pub pagination: Option<Pagination>,
}

/// Rendered home-page content from `content/home.md`, exposed to the
/// `home.html` template under the `home` variable. `content` is the
/// markdown body rendered to HTML through the same pipeline as every
/// other page — pass through `| safe` in the template.
#[derive(Debug, Clone, Serialize)]
pub struct HomeContent {
    pub content: String,
}

impl From<&Rendered> for HomeContent {
    fn from(rendered: &Rendered) -> Self {
        Self {
            content: rendered.html.clone(),
        }
    }
}

/// Context for the home page (`/`). Same posts as the blog index, plus
/// the optional rendered home-page content. `contains_mermaid` mirrors
/// the field on `PageContext` so `base.html` can use one check across
/// all page types.
#[derive(Debug, Serialize)]
pub struct HomeContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub posts: Vec<PostEntry>,
    pub home: Option<HomeContent>,
    pub contains_mermaid: bool,
}

/// A wiki page summary for the wiki index listing.
#[derive(Debug, Clone, Serialize)]
pub struct WikiEntry {
    pub title: String,
    pub url: String,
}

impl From<&Page<WikiFrontmatter>> for WikiEntry {
    fn from(page: &Page<WikiFrontmatter>) -> Self {
        Self {
            title: page.title().to_string(),
            url: page.url_path(),
        }
    }
}

/// A group of wiki pages sharing the same category.
#[derive(Debug, Clone, Serialize)]
pub struct WikiCategory {
    pub name: Option<String>,
    pub pages: Vec<WikiEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum WikiCategoryOrder {
    Configured(usize),
    Alphabetical(String),
    Uncategorized,
}

impl WikiCategoryOrder {
    fn from_name(name: Option<&str>, configured_order: &[String]) -> Self {
        match name {
            Some(name) => match configured_order
                .iter()
                .position(|configured| configured == name)
            {
                Some(index) => Self::Configured(index),
                None => Self::Alphabetical(name.to_owned()),
            },
            None => Self::Uncategorized,
        }
    }
}

impl WikiCategory {
    /// Group every wiki page in `site` by category. Pages within each
    /// category are sorted by title. Categories listed in
    /// `config.wiki_categories` come first in that order; named categories
    /// not listed fall through alphabetically; uncategorised pages last.
    pub fn from_site(site: &Site) -> Vec<Self> {
        let mut by_category: HashMap<Option<String>, Vec<WikiEntry>> = HashMap::new();
        for p in &site.wiki {
            by_category
                .entry(p.frontmatter.category.clone())
                .or_default()
                .push(WikiEntry::from(p));
        }
        for entries in by_category.values_mut() {
            entries.sort_by(|a, b| a.title.cmp(&b.title));
        }
        let mut categories: Vec<Self> = by_category
            .into_iter()
            .map(|(name, pages)| Self { name, pages })
            .collect();
        let order = &site.config.wiki_categories;
        categories.sort_by_cached_key(|category| {
            WikiCategoryOrder::from_name(category.name.as_deref(), order)
        });
        categories
    }
}

/// Context for the wiki index page (wiki pages grouped by category).
#[derive(Debug, Serialize)]
pub struct WikiIndexContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub categories: Vec<WikiCategory>,
}

/// Context for a single tag page (posts with that tag, paginated).
#[derive(Debug, Serialize)]
pub struct TagPageContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub tag: String,
    pub tag_slug: Slug,
    pub posts: Vec<PostEntry>,
    pub pagination: Option<Pagination>,
}

/// A tag summary for the tags index listing.
#[derive(Debug, Clone, Serialize)]
pub struct TagEntry {
    pub name: String,
    pub slug: Slug,
    pub count: usize,
}

impl TagEntry {
    pub fn new(name: &str, count: usize) -> Self {
        Self {
            name: name.to_owned(),
            slug: name.into(),
            count,
        }
    }
}

/// Context for the tags index page (list of all tags).
#[derive(Debug, Serialize)]
pub struct TagsIndexContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub tags: Vec<TagEntry>,
}

/// Context for the 404 page.
#[derive(Debug, Serialize)]
pub struct NotFoundContext {
    #[serde(flatten)]
    pub site: SiteContext,
}

/// Shared site-level fields present in every template context.
#[derive(Debug, Clone, Serialize)]
pub struct SiteContext {
    pub site_title: String,
    pub base_url: String,
    pub version: String,
    pub nav_pages: Vec<NavEntry>,
    pub socials: Vec<Social>,
    /// HTML `<link>` tags for favicons. Empty when no favicon is configured.
    /// Templates should render with `{{ favicon_tags | safe }}`.
    pub favicon_tags: String,
    /// Root-relative URL to the Atom feed (`/feed.xml`).
    pub feed_atom_url: String,
    /// Root-relative URL to the RSS feed (`/rss.xml`).
    pub feed_rss_url: String,
}

impl SiteContext {
    pub fn from_config(
        config: &Config,
        pages: &[Page<PageFrontmatter>],
        favicon: Option<&FaviconSet>,
    ) -> Self {
        Self {
            site_title: config.title.clone(),
            base_url: config.base_url.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            nav_pages: NavEntry::from_pages(pages),
            socials: config.socials.clone(),
            favicon_tags: favicon.map(|f| f.html_tags.clone()).unwrap_or_default(),
            feed_atom_url: "/feed.xml".into(),
            feed_rss_url: "/rss.xml".into(),
        }
    }
}

/// The full template context for rendering a single page.
#[derive(Debug, Serialize)]
pub struct PageContext {
    #[serde(flatten)]
    pub site: SiteContext,

    // Page-level
    pub title: String,
    pub url: String,
    pub kind: PageKind,
    pub content: String,
    pub toc: Vec<TocEntry>,
    pub backlinks: Vec<BacklinkEntry>,
    /// `true` when the page body contains at least one ` ```mermaid `
    /// block. Templates use this to load the Mermaid runtime only on the
    /// pages that need it.
    pub contains_mermaid: bool,

    // Wiki-specific (None / empty for blog/page)
    pub category: Option<String>,
    pub wiki_categories: Vec<WikiCategory>,

    // Blog-specific (None for wiki/page)
    pub author: Option<String>,
    pub image: Option<String>,
    pub description: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub tags: Vec<TagRef>,
    /// Adjacent post one step newer than this one in the blog feed.
    /// `None` on non-blog pages and on the newest blog post.
    pub newer_post: Option<PostEntry>,
    /// Adjacent post one step older than this one in the blog feed.
    /// `None` on non-blog pages and on the oldest blog post.
    pub older_post: Option<PostEntry>,
}

impl PageContext {
    pub fn from_page(
        page: &PageAny<'_>,
        rendered: &Rendered,
        site: &Site,
        site_ctx: &SiteContext,
        wiki_categories: &[WikiCategory],
    ) -> Self {
        let (newer_post, older_post) = blog_neighbours(page, site);
        Self {
            site: site_ctx.clone(),
            title: page.title().into_owned(),
            url: page.url_path(),
            kind: page.kind(),
            content: rendered.html.clone(),
            toc: TocEntry::from_headings(&rendered.toc),
            backlinks: site
                .backlinks_for(page.slug())
                .iter()
                .map(BacklinkEntry::from)
                .collect(),
            contains_mermaid: rendered.contains_mermaid,
            category: owned(page.category()),
            wiki_categories: match page.kind() {
                PageKind::Wiki => wiki_categories.to_vec(),
                _ => Vec::new(),
            },
            author: owned(page.author()),
            image: owned(page.image()),
            description: owned(page.description()),
            created: page.created(),
            updated: page.updated(),
            tags: TagRef::from_tags(page.tags()),
            newer_post,
            older_post,
        }
    }

    pub(super) fn template_name(&self) -> &'static str {
        self.kind.template_name()
    }
}

/// Find the posts one step newer and one step older than `page` in the
/// blog feed. Returns `(None, None)` for non-blog pages.
///
/// `Site::blog` is sorted newest-first, so the newer neighbour is at
/// index `i - 1` and the older at `i + 1`.
fn blog_neighbours(page: &PageAny<'_>, site: &Site) -> (Option<PostEntry>, Option<PostEntry>) {
    if page.kind() != PageKind::Blog {
        return (None, None);
    }
    let Some(idx) = site.blog.iter().position(|p| &p.slug == page.slug()) else {
        return (None, None);
    };
    let newer = idx
        .checked_sub(1)
        .and_then(|i| site.blog.get(i))
        .map(|p| PostEntry::from_page(&PageAny::Blog(p)));
    let older = site
        .blog
        .get(idx + 1)
        .map(|p| PostEntry::from_page(&PageAny::Blog(p)));
    (newer, older)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::content::WikiFrontmatter;
    use crate::content::page::Page;

    fn wiki_page(slug: &str, category: Option<&str>) -> Page<WikiFrontmatter> {
        Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/wiki/{slug}.md")),
            frontmatter: WikiFrontmatter {
                title: None,
                category: category.map(Into::into),
                created: None,
                updated: None,
                tags: vec![],
                draft: false,
            },
        }
    }

    fn site_with_wiki(wiki_categories: &[&str], pages: Vec<Page<WikiFrontmatter>>) -> Site {
        let mut config: Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        config.wiki_categories = wiki_categories.iter().map(|s| (*s).to_owned()).collect();
        Site::from_parts(config, vec![], pages, vec![]).unwrap()
    }

    #[test]
    fn wiki_categories_ordered_by_config_then_alphabetical_then_none() {
        let site = site_with_wiki(
            &["Getting Started", "Content"],
            vec![
                wiki_page("alpha", Some("Development")),   // unlisted-named
                wiki_page("beta", Some("Content")),        // listed second
                wiki_page("gamma", None),                  // uncategorised
                wiki_page("delta", Some("Customization")), // unlisted-named
                wiki_page("epsilon", Some("Getting Started")), // listed first
            ],
        );
        let cats = WikiCategory::from_site(&site);
        let names: Vec<_> = cats.iter().map(|c| c.name.as_deref()).collect();
        assert_eq!(
            names,
            vec![
                Some("Getting Started"),
                Some("Content"),
                Some("Customization"),
                Some("Development"),
                None,
            ]
        );
    }

    #[test]
    fn pagination_single_page_is_none() {
        assert!(Pagination::build("/blog/", 1, 1).is_none());
        assert!(Pagination::build("/blog/", 1, 0).is_none());
    }

    #[test]
    fn pagination_first_page_has_no_prev() {
        let p = Pagination::build("/blog/", 1, 3).unwrap();
        assert_eq!(p.current, 1);
        assert_eq!(p.total, 3);
        assert!(p.prev_url.is_none());
        assert_eq!(p.next_url.as_deref(), Some("/blog/page/2/"));
        let urls: Vec<_> = p.pages.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(urls, vec!["/blog/", "/blog/page/2/", "/blog/page/3/"]);
    }

    #[test]
    fn pagination_last_page_has_no_next() {
        let p = Pagination::build("/blog/", 3, 3).unwrap();
        assert_eq!(p.prev_url.as_deref(), Some("/blog/page/2/"));
        assert!(p.next_url.is_none());
    }

    #[test]
    fn pagination_middle_page_has_both() {
        let p = Pagination::build("/blog/", 2, 3).unwrap();
        assert_eq!(p.prev_url.as_deref(), Some("/blog/"));
        assert_eq!(p.next_url.as_deref(), Some("/blog/page/3/"));
    }

    fn blog_post_for_test(slug: &str, day: u32) -> Page<crate::content::BlogFrontmatter> {
        use crate::content::BlogFrontmatter;
        use chrono::NaiveDate;
        Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/blog/{slug}.md")),
            frontmatter: BlogFrontmatter {
                title: format!("Post {slug}"),
                slug: slug.into(),
                author: "T".into(),
                created: NaiveDate::from_ymd_opt(2026, 1, day).unwrap(),
                updated: None,
                image: None,
                description: None,
                tags: vec![],
                draft: false,
            },
        }
    }

    fn site_with_blog(posts: Vec<Page<crate::content::BlogFrontmatter>>) -> Site {
        let config: Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        let mut site = Site::from_parts(config, posts, vec![], vec![]).unwrap();
        site.blog.sort();
        site
    }

    #[test]
    fn blog_neighbours_for_middle_post() {
        let site = site_with_blog(vec![
            blog_post_for_test("a", 1),
            blog_post_for_test("b", 2),
            blog_post_for_test("c", 3),
        ]);
        let middle = PageAny::Blog(site.blog.iter().find(|p| p.slug.as_ref() == "b").unwrap());
        let (newer, older) = blog_neighbours(&middle, &site);
        assert_eq!(newer.unwrap().title, "Post c");
        assert_eq!(older.unwrap().title, "Post a");
    }

    #[test]
    fn blog_neighbours_newest_has_no_newer() {
        let site = site_with_blog(vec![blog_post_for_test("a", 1), blog_post_for_test("b", 2)]);
        let newest = PageAny::Blog(&site.blog[0]);
        let (newer, older) = blog_neighbours(&newest, &site);
        assert!(newer.is_none());
        assert!(older.is_some());
    }

    #[test]
    fn blog_neighbours_oldest_has_no_older() {
        let site = site_with_blog(vec![blog_post_for_test("a", 1), blog_post_for_test("b", 2)]);
        let oldest = PageAny::Blog(&site.blog[1]);
        let (newer, older) = blog_neighbours(&oldest, &site);
        assert!(newer.is_some());
        assert!(older.is_none());
    }

    #[test]
    fn blog_neighbours_single_post_has_no_neighbours() {
        let site = site_with_blog(vec![blog_post_for_test("only", 1)]);
        let only = PageAny::Blog(&site.blog[0]);
        let (newer, older) = blog_neighbours(&only, &site);
        assert!(newer.is_none());
        assert!(older.is_none());
    }

    #[test]
    fn blog_neighbours_returns_none_for_wiki_page() {
        let site = site_with_blog(vec![blog_post_for_test("a", 1)]);
        let wiki = wiki_page("w", None);
        let any = PageAny::Wiki(&wiki);
        let (newer, older) = blog_neighbours(&any, &site);
        assert!(newer.is_none());
        assert!(older.is_none());
    }

    #[test]
    fn pagination_works_for_tag_base_path() {
        let p = Pagination::build("/tags/rust/", 1, 2).unwrap();
        assert_eq!(p.next_url.as_deref(), Some("/tags/rust/page/2/"));
        let urls: Vec<_> = p.pages.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(urls, vec!["/tags/rust/", "/tags/rust/page/2/"]);
    }

    #[test]
    fn wiki_categories_default_to_alphabetical_when_config_empty() {
        let site = site_with_wiki(
            &[],
            vec![
                wiki_page("a", Some("Zeta")),
                wiki_page("b", Some("Alpha")),
                wiki_page("c", None),
            ],
        );
        let cats = WikiCategory::from_site(&site);
        let names: Vec<_> = cats.iter().map(|c| c.name.as_deref()).collect();
        assert_eq!(names, vec![Some("Alpha"), Some("Zeta"), None]);
    }
}
