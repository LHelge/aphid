use super::frontmatter::{BlogFrontmatter, PageFrontmatter, WikiFrontmatter};
use super::slug::Slug;
use std::borrow::Cow;
use std::path::PathBuf;

use serde::Serialize;

/// A loaded markdown page, generic over its frontmatter type. The `body`
/// has already had its YAML frontmatter stripped at load time.
pub struct Page<F> {
    pub slug: Slug,
    pub body: String,
    pub path: PathBuf,
    pub frontmatter: F,
}

/// Which kind of page a given slug resolves to. Determines the URL
/// scheme (`/blog/foo/`, `/wiki/foo/`, `/foo/`) and the template used
/// for rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PageKind {
    Blog,
    Wiki,
    Page,
}

impl PageKind {
    pub fn url_path(&self, slug: &Slug) -> String {
        match self {
            PageKind::Blog => format!("/blog/{slug}/"),
            PageKind::Wiki => format!("/wiki/{slug}/"),
            PageKind::Page => format!("/{slug}/"),
        }
    }

    pub fn template_name(&self) -> &'static str {
        match self {
            PageKind::Blog => "blog_post.html",
            PageKind::Wiki => "wiki_page.html",
            PageKind::Page => "page.html",
        }
    }
}

impl Page<BlogFrontmatter> {
    pub fn title(&self) -> &str {
        &self.frontmatter.title
    }
    pub fn url_path(&self) -> String {
        PageKind::Blog.url_path(&self.slug)
    }
}

impl PartialEq for Page<BlogFrontmatter> {
    fn eq(&self, other: &Self) -> bool {
        self.frontmatter.created == other.frontmatter.created && self.slug == other.slug
    }
}

impl Eq for Page<BlogFrontmatter> {}

impl PartialOrd for Page<BlogFrontmatter> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Page<BlogFrontmatter> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .frontmatter
            .created
            .cmp(&self.frontmatter.created)
            .then_with(|| self.slug.cmp(&other.slug))
    }
}

impl Page<WikiFrontmatter> {
    pub fn title(&self) -> Cow<'_, str> {
        match &self.frontmatter.title {
            Some(t) => Cow::Borrowed(t.as_str()),
            None => Cow::Owned(self.slug.to_title()),
        }
    }
    pub fn url_path(&self) -> String {
        PageKind::Wiki.url_path(&self.slug)
    }
}

impl Page<PageFrontmatter> {
    pub fn title(&self) -> &str {
        &self.frontmatter.title
    }
    pub fn url_path(&self) -> String {
        PageKind::Page.url_path(&self.slug)
    }
}

/// Borrowed reference to any kind of page. Used by code that operates
/// on pages uniformly (e.g. backlink lookup, wiki-link resolution) without
/// caring which subtype the slug resolves to.
pub enum PageAny<'a> {
    Blog(&'a Page<BlogFrontmatter>),
    Wiki(&'a Page<WikiFrontmatter>),
    Page(&'a Page<PageFrontmatter>),
}

impl<'a> PageAny<'a> {
    pub fn slug(&self) -> &Slug {
        match self {
            PageAny::Blog(p) => &p.slug,
            PageAny::Wiki(p) => &p.slug,
            PageAny::Page(p) => &p.slug,
        }
    }

    pub fn kind(&self) -> PageKind {
        match self {
            PageAny::Blog(_) => PageKind::Blog,
            PageAny::Wiki(_) => PageKind::Wiki,
            PageAny::Page(_) => PageKind::Page,
        }
    }

    pub fn url_path(&self) -> String {
        match self {
            PageAny::Blog(p) => p.url_path(),
            PageAny::Wiki(p) => p.url_path(),
            PageAny::Page(p) => p.url_path(),
        }
    }

    pub fn title(&self) -> Cow<'a, str> {
        match self {
            PageAny::Blog(p) => Cow::Borrowed(p.title()),
            PageAny::Wiki(p) => p.title(),
            PageAny::Page(p) => Cow::Borrowed(p.title()),
        }
    }

    pub fn author(&self) -> Option<&str> {
        match self {
            PageAny::Blog(p) => Some(&p.frontmatter.author),
            _ => None,
        }
    }

    pub fn image(&self) -> Option<&str> {
        match self {
            PageAny::Blog(p) => p.frontmatter.image.as_deref(),
            _ => None,
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            PageAny::Blog(p) => p.frontmatter.description.as_deref(),
            _ => None,
        }
    }

    pub fn category(&self) -> Option<&str> {
        match self {
            PageAny::Wiki(p) => p.frontmatter.category.as_deref(),
            _ => None,
        }
    }

    pub fn created(&self) -> Option<String> {
        match self {
            PageAny::Blog(p) => Some(p.frontmatter.created.to_string()),
            PageAny::Wiki(p) => p.frontmatter.created.map(|d| d.to_string()),
            PageAny::Page(_) => None,
        }
    }

    pub fn updated(&self) -> Option<String> {
        match self {
            PageAny::Blog(p) => p.frontmatter.updated.map(|d| d.to_string()),
            PageAny::Wiki(p) => p.frontmatter.updated.map(|d| d.to_string()),
            PageAny::Page(_) => None,
        }
    }

    pub fn tags(&self) -> &[String] {
        match self {
            PageAny::Blog(p) => &p.frontmatter.tags,
            PageAny::Wiki(p) => &p.frontmatter.tags,
            PageAny::Page(_) => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn make_blog_page(slug: &str) -> Page<BlogFrontmatter> {
        Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/blog/{slug}.md")),
            frontmatter: BlogFrontmatter {
                title: "Test Post".into(),
                slug: slug.into(),
                author: "Alice".into(),
                created: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                updated: None,
                image: None,
                description: None,
                tags: vec![],
            },
        }
    }

    fn make_wiki_page(slug: &str, title: Option<&str>) -> Page<WikiFrontmatter> {
        Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/wiki/{slug}.md")),
            frontmatter: WikiFrontmatter {
                title: title.map(Into::into),
                category: None,
                created: None,
                updated: None,
                tags: vec![],
            },
        }
    }

    fn make_standalone_page(slug: &str) -> Page<PageFrontmatter> {
        Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/pages/{slug}.md")),
            frontmatter: PageFrontmatter {
                title: "About".into(),
                order: None,
            },
        }
    }

    #[test]
    fn url_path_blog() {
        let slug = Slug::from("hello-world");
        assert_eq!(PageKind::Blog.url_path(&slug), "/blog/hello-world/");
    }

    #[test]
    fn url_path_wiki() {
        let slug = Slug::from("glossary");
        assert_eq!(PageKind::Wiki.url_path(&slug), "/wiki/glossary/");
    }

    #[test]
    fn url_path_page() {
        let slug = Slug::from("about");
        assert_eq!(PageKind::Page.url_path(&slug), "/about/");
    }

    #[test]
    fn page_url_paths_agree_with_page_kind() {
        let blog = make_blog_page("hello-world");
        assert_eq!(blog.url_path(), PageKind::Blog.url_path(&blog.slug));

        let wiki = make_wiki_page("glossary", None);
        assert_eq!(wiki.url_path(), PageKind::Wiki.url_path(&wiki.slug));

        let page = make_standalone_page("about");
        assert_eq!(page.url_path(), PageKind::Page.url_path(&page.slug));
    }

    #[test]
    fn page_any_url_path_matches_concrete() {
        let blog = make_blog_page("hello-world");
        let any = PageAny::Blog(&blog);
        assert_eq!(any.url_path(), blog.url_path());

        let wiki = make_wiki_page("glossary", None);
        let any = PageAny::Wiki(&wiki);
        assert_eq!(any.url_path(), wiki.url_path());

        let page = make_standalone_page("about");
        let any = PageAny::Page(&page);
        assert_eq!(any.url_path(), page.url_path());
    }

    #[test]
    fn wiki_title_from_frontmatter() {
        let page = make_wiki_page("glossary", Some("Glossary Override"));
        assert_eq!(page.title(), Cow::Borrowed("Glossary Override"));
    }

    #[test]
    fn wiki_title_derived_from_slug() {
        let page = make_wiki_page("battery-pack", None);
        assert_eq!(page.title(), Cow::<str>::Owned("Battery Pack".into()));
    }

    #[test]
    fn page_any_image_only_set_for_blog() {
        let mut blog = make_blog_page("hello-world");
        blog.frontmatter.image = Some("/static/blog/hero.png".into());
        assert_eq!(PageAny::Blog(&blog).image(), Some("/static/blog/hero.png"));

        let blog_no_image = make_blog_page("plain");
        assert_eq!(PageAny::Blog(&blog_no_image).image(), None);

        let wiki = make_wiki_page("glossary", None);
        assert_eq!(PageAny::Wiki(&wiki).image(), None);

        let standalone = make_standalone_page("about");
        assert_eq!(PageAny::Page(&standalone).image(), None);
    }
}
