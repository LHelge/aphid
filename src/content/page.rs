use super::frontmatter::{BlogFrontmatter, PageFrontmatter, WikiFrontmatter};
use super::slug::Slug;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

/// The minimum interface every page exposes regardless of kind. Used by
/// kind-erased callers (slug → page lookup, backlink targets, wiki-link
/// resolution) that only need identity-shaped data. Kind-specific
/// metadata is reached through the concrete `Page<F>` type.
pub trait PageView {
    fn slug(&self) -> &Slug;
    fn title(&self) -> &str;
    fn kind(&self) -> PageKind;

    fn url_path(&self) -> String {
        self.kind().url_path(self.slug())
    }
}

impl PageView for Page<BlogFrontmatter> {
    fn slug(&self) -> &Slug {
        &self.slug
    }
    fn title(&self) -> &str {
        &self.frontmatter.title
    }
    fn kind(&self) -> PageKind {
        PageKind::Blog
    }
}

impl PageView for Page<WikiFrontmatter> {
    fn slug(&self) -> &Slug {
        &self.slug
    }
    fn title(&self) -> &str {
        // Loader resolves an empty frontmatter title to the slug-derived
        // form, so this is non-empty for any wiki page that came through
        // `Site::load`.
        &self.frontmatter.title
    }
    fn kind(&self) -> PageKind {
        PageKind::Wiki
    }
}

impl PageView for Page<PageFrontmatter> {
    fn slug(&self) -> &Slug {
        &self.slug
    }
    fn title(&self) -> &str {
        &self.frontmatter.title
    }
    fn kind(&self) -> PageKind {
        PageKind::Page
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
                draft: false,
            },
        }
    }

    fn make_wiki_page(slug: &str, title: Option<&str>) -> Page<WikiFrontmatter> {
        let slug: Slug = slug.into();
        let resolved_title = title.map(str::to_owned).unwrap_or_else(|| slug.to_title());
        Page {
            slug: slug.clone(),
            body: String::new(),
            path: PathBuf::from(format!("content/wiki/{slug}.md")),
            frontmatter: WikiFrontmatter {
                title: resolved_title,
                category: None,
                created: None,
                updated: None,
                tags: vec![],
                draft: false,
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
                draft: false,
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
    fn page_view_url_path_matches_page_kind() {
        let blog = make_blog_page("hello-world");
        assert_eq!(blog.url_path(), PageKind::Blog.url_path(&blog.slug));

        let wiki = make_wiki_page("glossary", None);
        assert_eq!(wiki.url_path(), PageKind::Wiki.url_path(&wiki.slug));

        let page = make_standalone_page("about");
        assert_eq!(page.url_path(), PageKind::Page.url_path(&page.slug));
    }

    #[test]
    fn page_view_kind_matches_concrete_type() {
        let blog = make_blog_page("hello-world");
        assert_eq!(blog.kind(), PageKind::Blog);

        let wiki = make_wiki_page("glossary", None);
        assert_eq!(wiki.kind(), PageKind::Wiki);

        let page = make_standalone_page("about");
        assert_eq!(page.kind(), PageKind::Page);
    }

    #[test]
    fn wiki_title_from_frontmatter() {
        let page = make_wiki_page("glossary", Some("Glossary Override"));
        assert_eq!(page.title(), "Glossary Override");
    }

    #[test]
    fn wiki_title_derived_from_slug() {
        let page = make_wiki_page("battery-pack", None);
        assert_eq!(page.title(), "Battery Pack");
    }
}
