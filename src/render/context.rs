use std::collections::HashMap;

use serde::Serialize;

use crate::config::{Config, Social};
use crate::content::page::{Page, PageKind};
use crate::content::slug::Slug;
use crate::content::{BlogFrontmatter, PageFrontmatter, PageView, Site, WikiFrontmatter};
use crate::favicon::FaviconSet;
use crate::markdown::{HeadingEntry, Rendered};

/// Author metadata resolved from config, exposed to blog post templates.
///
/// The frontmatter `author` string is looked up against `[[authors]]` in
/// `aphid.toml`. If found, the full author record (link, image) is used;
/// otherwise only the name survives.
#[derive(Debug, Clone, Serialize)]
pub struct AuthorContext {
    pub name: String,
    pub link: Option<String>,
    pub image: Option<String>,
}

impl AuthorContext {
    /// Build an `AuthorContext` by looking up the given name in the config
    /// authors list. The author's `image` path is exposed verbatim — write
    /// it as a root-relative URL (`/static/authors/alice.jpg`) or an
    /// absolute one, matching the convention used for blog hero images
    /// and the `favicon` config field.
    pub fn resolve(name: &str, site: &Site) -> Self {
        let author = site.config.authors.iter().find(|a| a.name == name);

        match author {
            Some(a) => Self {
                name: a.name.clone(),
                link: a
                    .link
                    .clone()
                    .or_else(|| a.email.as_ref().map(|email| format!("mailto:{email}"))),
                image: a.image.clone(),
            },
            None => Self {
                name: name.to_owned(),
                link: None,
                image: None,
            },
        }
    }
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

impl BacklinkEntry {
    fn from_view(page: &dyn PageView) -> Self {
        Self {
            title: page.title().to_string(),
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

/// A page summary for index/tag listing pages. Carries the union of
/// fields any kind of listed page might want to show — blog posts
/// populate everything; wiki pages contribute what they have and leave
/// the blog-specific fields empty.
#[derive(Debug, Clone, Serialize)]
pub struct PostEntry {
    pub title: String,
    pub url: String,
    pub created: Option<String>,
    pub image: Option<String>,
    pub description: Option<String>,
    pub reading_time_minutes: u32,
    pub tags: Vec<TagRef>,
}

impl PostEntry {
    pub fn from_blog_page(page: &Page<BlogFrontmatter>, wpm: u32) -> Self {
        Self {
            title: page.frontmatter.title.clone(),
            url: page.url_path(),
            created: Some(page.frontmatter.created.to_string()),
            image: page.frontmatter.image.clone(),
            description: page.frontmatter.description.clone(),
            reading_time_minutes: page.reading_time_minutes(wpm),
            tags: TagRef::from_tags(&page.frontmatter.tags),
        }
    }

    pub fn from_wiki_page(page: &Page<WikiFrontmatter>, wpm: u32) -> Self {
        Self {
            title: page.frontmatter.title.clone(),
            url: page.url_path(),
            created: page.frontmatter.created.map(|d| d.to_string()),
            image: None,
            description: None,
            reading_time_minutes: page.reading_time_minutes(wpm),
            tags: TagRef::from_tags(&page.frontmatter.tags),
        }
    }

    pub fn from_blog_pages<'a>(
        pages: impl IntoIterator<Item = &'a Page<BlogFrontmatter>>,
        wpm: u32,
    ) -> Vec<Self> {
        pages
            .into_iter()
            .map(|p| Self::from_blog_page(p, wpm))
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

/// Rendered 404-page content from `content/404.md`, exposed to the
/// `404.html` template under the `not_found` variable. Same shape and
/// rules as [`HomeContent`] — themes use it as
/// `{% if not_found %}{{ not_found.content | safe }}{% endif %}`.
#[derive(Debug, Clone, Serialize)]
pub struct NotFoundContent {
    pub content: String,
}

impl From<&Rendered> for NotFoundContent {
    fn from(rendered: &Rendered) -> Self {
        Self {
            content: rendered.html.clone(),
        }
    }
}

/// Context for the home page (`/`). Same posts as the blog index, plus
/// the optional rendered home-page content. `contains_mermaid` mirrors
/// the field on the page contexts so `base.html` can use one check
/// across all page types.
#[derive(Debug, Serialize)]
pub struct HomeContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub posts: Vec<PostEntry>,
    pub home: Option<HomeContent>,
    pub contains_mermaid: bool,
    /// Every tag used across blog and wiki content, sorted by descending
    /// count and then ascending name. Themes use this to render a tag
    /// cloud on the home page; counts match the `/tags/` index.
    pub popular_tags: Vec<TagEntry>,
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
            title: page.frontmatter.title.clone(),
            url: page.url_path(),
        }
    }
}

/// A group of wiki pages sharing the same category. Pages without an
/// explicit category in frontmatter land under
/// `Config::wiki_default_category` ("Other" by default), so this
/// `name` is always populated.
#[derive(Debug, Clone, Serialize)]
pub struct WikiCategory {
    pub name: String,
    pub pages: Vec<WikiEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum WikiCategoryOrder {
    Configured(usize),
    Alphabetical(String),
    Default,
}

impl WikiCategoryOrder {
    fn from_name(name: &str, default_name: &str, configured_order: &[String]) -> Self {
        if name == default_name {
            return Self::Default;
        }
        match configured_order.iter().position(|c| c == name) {
            Some(i) => Self::Configured(i),
            None => Self::Alphabetical(name.to_owned()),
        }
    }
}

impl WikiCategory {
    /// Group every wiki page in `site` by category. Pages within each
    /// category are sorted by title. Categories listed in
    /// `config.wiki_categories` come first in that order; other named
    /// categories fall through alphabetically; the default catch-all
    /// category (config.wiki_default_category) sorts last.
    pub fn from_site(site: &Site) -> Vec<Self> {
        let default = &site.config.wiki_default_category;
        let mut by_category: HashMap<String, Vec<WikiEntry>> = HashMap::new();
        for p in &site.wiki {
            let name = p
                .frontmatter
                .category
                .clone()
                .unwrap_or_else(|| default.clone());
            by_category
                .entry(name)
                .or_default()
                .push(WikiEntry::from(p));
        }
        for entries in by_category.values_mut() {
            entries.sort_by(|a, b| a.title.cmp(&b.title));
        }
        let order = &site.config.wiki_categories;
        let mut categories: Vec<Self> = by_category
            .into_iter()
            .map(|(name, pages)| Self { name, pages })
            .collect();
        categories
            .sort_by_cached_key(|cat| WikiCategoryOrder::from_name(&cat.name, default, order));
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
///
/// Natural ordering is "popularity": descending `count` first, then
/// ascending `name` to break ties alphabetically. Callers that want
/// alphabetical order (e.g. the `/tags/` index) sort by `name`
/// explicitly via `sort_by`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

    /// Every tag across blog and wiki content, sorted by popularity
    /// (descending count, ties broken by ascending name). Counts match
    /// the `/tags/` index — a tag's number is identical wherever it
    /// appears on the site.
    pub fn popular_from_site(site: &Site) -> Vec<Self> {
        let mut tags: Vec<Self> = site
            .tag_index
            .iter()
            .map(|(name, slugs)| Self::new(name, slugs.len()))
            .collect();
        tags.sort();
        tags
    }
}

impl Ord for TagEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .count
            .cmp(&self.count)
            .then_with(|| self.name.cmp(&other.name))
    }
}

impl PartialOrd for TagEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Context for the tags index page (list of all tags).
#[derive(Debug, Serialize)]
pub struct TagsIndexContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub tags: Vec<TagEntry>,
}

/// Context for the 404 page. `not_found` is populated when
/// `content/404.md` exists; otherwise themes fall back to whatever copy
/// they ship hardcoded. `contains_mermaid` mirrors the field on other
/// page contexts so `base.html` can use one check across all page types.
#[derive(Debug, Serialize)]
pub struct NotFoundContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub not_found: Option<NotFoundContent>,
    pub contains_mermaid: bool,
}

/// Shared site-level fields present in every template context.
#[derive(Debug, Clone, Serialize)]
pub struct SiteContext {
    pub site_title: String,
    /// Site-wide description from config, exposed for SEO `<meta>` tags
    /// and as the OpenGraph description fallback on pages without their
    /// own.
    pub site_description: Option<String>,
    /// Absolute URL for the site-wide default OpenGraph / Twitter card
    /// image. `None` when no `social_image` is configured.
    pub social_image_url: Option<String>,
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
        let social_image_url = config
            .social_image
            .as_deref()
            .map(|p| config.absolute_url(p).into());
        Self {
            site_title: config.title.clone(),
            site_description: config.description.clone(),
            social_image_url,
            version: env!("CARGO_PKG_VERSION").to_string(),
            nav_pages: NavEntry::from_pages(pages),
            socials: config.socials.clone(),
            favicon_tags: favicon.map(|f| f.html_tags.clone()).unwrap_or_default(),
            feed_atom_url: "/feed.xml".into(),
            feed_rss_url: "/rss.xml".into(),
        }
    }
}

/// The base context every page template receives: shared site fields
/// plus the universal page fields (title, URL, rendered body, table of
/// contents, mermaid flag). Standalone pages render with this directly;
/// wiki and blog page contexts embed it via `#[serde(flatten)]`.
#[derive(Debug, Clone, Serialize)]
pub struct StandalonePageContext {
    #[serde(flatten)]
    pub site: SiteContext,
    pub title: String,
    /// Root-relative URL of this page (e.g. `/blog/my-post/`).
    pub url: String,
    /// Absolute URL of this page (`base_url` joined with `url`). Pre-built
    /// so templates never have to assemble canonical URLs from pieces —
    /// they just emit `{{ canonical_url }}` for OpenGraph tags and similar.
    pub canonical_url: String,
    pub content: String,
    pub toc: Vec<TocEntry>,
    pub contains_mermaid: bool,
}

impl StandalonePageContext {
    pub fn from_page(
        page: &Page<PageFrontmatter>,
        rendered: &Rendered,
        site: &Site,
        site_ctx: &SiteContext,
    ) -> Self {
        let url = page.url_path();
        Self {
            site: site_ctx.clone(),
            title: page.frontmatter.title.clone(),
            canonical_url: site.config.absolute_url(&url).into(),
            url,
            content: rendered.html.clone(),
            toc: TocEntry::from_headings(&rendered.toc),
            contains_mermaid: rendered.contains_mermaid,
        }
    }
}

/// Template context for a single wiki page.
#[derive(Debug, Serialize)]
pub struct WikiPageContext {
    #[serde(flatten)]
    pub base: StandalonePageContext,
    pub category: String,
    pub backlinks: Vec<BacklinkEntry>,
    pub wiki_categories: Vec<WikiCategory>,
}

impl WikiPageContext {
    pub fn from_page(
        page: &Page<WikiFrontmatter>,
        rendered: &Rendered,
        site: &Site,
        site_ctx: &SiteContext,
        wiki_categories: Vec<WikiCategory>,
    ) -> Self {
        let category = page
            .frontmatter
            .category
            .clone()
            .unwrap_or_else(|| site.config.wiki_default_category.clone());
        let backlinks = site
            .backlinks_for(&page.slug)
            .into_iter()
            .map(BacklinkEntry::from_view)
            .collect();
        let url = page.url_path();
        let base = StandalonePageContext {
            site: site_ctx.clone(),
            title: page.frontmatter.title.clone(),
            canonical_url: site.config.absolute_url(&url).into(),
            url,
            content: rendered.html.clone(),
            toc: TocEntry::from_headings(&rendered.toc),
            contains_mermaid: rendered.contains_mermaid,
        };
        Self {
            base,
            category,
            backlinks,
            wiki_categories,
        }
    }
}

/// Template context for a single blog post.
#[derive(Debug, Serialize)]
pub struct BlogPostContext {
    #[serde(flatten)]
    pub base: StandalonePageContext,
    pub author: AuthorContext,
    pub image: Option<String>,
    /// Absolute URL for the post's OpenGraph / Twitter card image —
    /// the frontmatter `image` made absolute, or the site `social_image`
    /// fallback. `None` when neither is set.
    pub og_image: Option<String>,
    pub description: Option<String>,
    pub created: String,
    pub updated: Option<String>,
    /// Rough reading-time estimate for the post body, in minutes (rounded
    /// up, minimum 1). Templates typically render as "X min read".
    pub reading_time_minutes: u32,
    pub tags: Vec<TagRef>,
    /// Adjacent post one step newer than this one in the blog feed.
    /// `None` on the newest blog post.
    pub newer_post: Option<PostEntry>,
    /// Adjacent post one step older than this one in the blog feed.
    /// `None` on the oldest blog post.
    pub older_post: Option<PostEntry>,
}

impl BlogPostContext {
    pub fn from_page(
        page: &Page<BlogFrontmatter>,
        rendered: &Rendered,
        site: &Site,
        site_ctx: &SiteContext,
    ) -> Self {
        let (newer, older) = site.blog_neighbours_of(page);
        let og_image = page
            .frontmatter
            .image
            .as_deref()
            .map(|img| site.config.absolute_url(img).into())
            .or_else(|| site_ctx.social_image_url.clone());
        let url = page.url_path();
        let base = StandalonePageContext {
            site: site_ctx.clone(),
            title: page.frontmatter.title.clone(),
            canonical_url: site.config.absolute_url(&url).into(),
            url,
            content: rendered.html.clone(),
            toc: TocEntry::from_headings(&rendered.toc),
            contains_mermaid: rendered.contains_mermaid,
        };
        Self {
            base,
            author: AuthorContext::resolve(&page.frontmatter.author, site),
            image: page.frontmatter.image.clone(),
            og_image,
            description: page.frontmatter.description.clone(),
            created: page.frontmatter.created.to_string(),
            updated: page.frontmatter.updated.map(|d| d.to_string()),
            reading_time_minutes: page.reading_time_minutes(site.config.reading_wpm),
            tags: TagRef::from_tags(&page.frontmatter.tags),
            newer_post: newer.map(|p| PostEntry::from_blog_page(p, site.config.reading_wpm)),
            older_post: older.map(|p| PostEntry::from_blog_page(p, site.config.reading_wpm)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::content::WikiFrontmatter;
    use crate::content::page::Page;

    fn wiki_page(slug: &str, category: Option<&str>) -> Page<WikiFrontmatter> {
        let slug: Slug = slug.into();
        Page {
            slug: slug.clone(),
            body: String::new(),
            path: PathBuf::from(format!("content/wiki/{slug}.md")),
            frontmatter: WikiFrontmatter {
                title: slug.to_title(),
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
    fn wiki_categories_ordered_by_config_then_alphabetical_then_default() {
        let site = site_with_wiki(
            &["Getting Started", "Content"],
            vec![
                wiki_page("alpha", Some("Development")),   // unlisted-named
                wiki_page("beta", Some("Content")),        // listed second
                wiki_page("gamma", None),                  // default-bucket
                wiki_page("delta", Some("Customization")), // unlisted-named
                wiki_page("epsilon", Some("Getting Started")), // listed first
            ],
        );
        let cats = WikiCategory::from_site(&site);
        let names: Vec<_> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Getting Started",
                "Content",
                "Customization",
                "Development",
                "Other", // default name for the catch-all
            ]
        );
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
        let names: Vec<_> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "Zeta", "Other"]);
    }

    #[test]
    fn wiki_default_category_is_configurable() {
        let mut config: Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        config.wiki_default_category = "Misc".into();
        let site = Site::from_parts(config, vec![], vec![wiki_page("a", None)], vec![]).unwrap();
        let cats = WikiCategory::from_site(&site);
        assert_eq!(cats[0].name, "Misc");
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
        let middle = site.blog.iter().find(|p| p.slug.as_ref() == "b").unwrap();
        let (newer, older) = site.blog_neighbours_of(middle);
        assert_eq!(newer.unwrap().frontmatter.title, "Post c");
        assert_eq!(older.unwrap().frontmatter.title, "Post a");
    }

    #[test]
    fn blog_neighbours_newest_has_no_newer() {
        let site = site_with_blog(vec![blog_post_for_test("a", 1), blog_post_for_test("b", 2)]);
        let (newer, older) = site.blog_neighbours_of(&site.blog[0]);
        assert!(newer.is_none());
        assert!(older.is_some());
    }

    #[test]
    fn blog_neighbours_oldest_has_no_older() {
        let site = site_with_blog(vec![blog_post_for_test("a", 1), blog_post_for_test("b", 2)]);
        let (newer, older) = site.blog_neighbours_of(&site.blog[1]);
        assert!(newer.is_some());
        assert!(older.is_none());
    }

    #[test]
    fn blog_neighbours_single_post_has_no_neighbours() {
        let site = site_with_blog(vec![blog_post_for_test("only", 1)]);
        let (newer, older) = site.blog_neighbours_of(&site.blog[0]);
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

    fn site_with_author_toml(author_toml: &str) -> Site {
        let config: Config =
            format!("title = \"T\"\nbase_url = \"http://x\"\n\n[[authors]]\n{author_toml}\n")
                .parse()
                .unwrap();
        Site::from_parts(config, vec![], vec![], vec![]).unwrap()
    }

    #[test]
    fn author_link_used_verbatim_when_set() {
        let site = site_with_author_toml(
            "name = \"Alice\"\nlink = \"https://alice.example.com\"\nemail = \"alice@example.com\"",
        );
        let ctx = AuthorContext::resolve("Alice", &site);
        assert_eq!(ctx.link.as_deref(), Some("https://alice.example.com"));
    }

    #[test]
    fn author_email_falls_back_to_mailto() {
        let site = site_with_author_toml("name = \"Alice\"\nemail = \"alice@example.com\"");
        let ctx = AuthorContext::resolve("Alice", &site);
        assert_eq!(ctx.link.as_deref(), Some("mailto:alice@example.com"));
    }

    #[test]
    fn author_with_no_link_or_email_has_none() {
        let site = site_with_author_toml("name = \"Alice\"");
        let ctx = AuthorContext::resolve("Alice", &site);
        assert!(ctx.link.is_none());
    }

    #[test]
    fn unknown_author_resolves_to_name_only() {
        let site = site_with_author_toml("name = \"Alice\"\nemail = \"alice@example.com\"");
        let ctx = AuthorContext::resolve("Bob", &site);
        assert_eq!(ctx.name, "Bob");
        assert!(ctx.link.is_none());
        assert!(ctx.image.is_none());
    }

    #[test]
    fn tag_entry_sorts_by_count_desc_then_name_asc() {
        let mut tags = [
            TagEntry::new("rust", 2),
            TagEntry::new("apple", 5),
            TagEntry::new("banana", 5),
            TagEntry::new("zebra", 1),
        ];
        tags.sort();
        let order: Vec<_> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(order, vec!["apple", "banana", "rust", "zebra"]);
    }

    fn tagged_blog_post(
        slug: &str,
        day: u32,
        tags: &[&str],
    ) -> Page<crate::content::BlogFrontmatter> {
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
                tags: tags.iter().map(|s| (*s).to_owned()).collect(),
                draft: false,
            },
        }
    }

    #[test]
    fn popular_from_site_returns_tags_in_popularity_order() {
        let posts = vec![
            tagged_blog_post("a", 1, &["rust", "cli"]),
            tagged_blog_post("b", 2, &["rust"]),
            tagged_blog_post("c", 3, &["rust", "cli", "web"]),
        ];
        let config: Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        let site = Site::from_parts(config, posts, vec![], vec![]).unwrap();
        let tags = TagEntry::popular_from_site(&site);
        let pairs: Vec<_> = tags.iter().map(|t| (t.name.as_str(), t.count)).collect();
        assert_eq!(pairs, vec![("rust", 3), ("cli", 2), ("web", 1)]);
    }

    #[test]
    fn popular_from_site_empty_when_no_tags() {
        let config: Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        let site = Site::from_parts(config, vec![], vec![], vec![]).unwrap();
        assert!(TagEntry::popular_from_site(&site).is_empty());
    }
}
