use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{fs, io};

use serde::de::DeserializeOwned;

use super::frontmatter::{self, BlogFrontmatter, PageFrontmatter, WikiFrontmatter};
use super::page::{Page, PageKind, PageView};
use super::slug::Slug;
use crate::Error;
use crate::config::Config;
use crate::markdown::wikilinks::extract_wiki_links;

/// Optional home-page content loaded from `content/home.md`. Renders into
/// the `home.html` template's body slot, between any hero and the blog
/// listing. The body is plain Markdown — no frontmatter — and runs through
/// the same render pipeline as every other page (wiki-links, heading
/// anchors, syntax highlighting all work).
pub struct HomePage {
    pub body: String,
    pub path: PathBuf,
}

fn load_home(path: &Path) -> Result<Option<HomePage>, Error> {
    if !path.is_file() {
        return Ok(None);
    }
    let body = fs::read_to_string(path).map_err(|e| {
        if e.kind() == io::ErrorKind::InvalidData {
            return Error::NotUtf8 {
                path: path.to_path_buf(),
            };
        }
        Error::LoadPage {
            path: path.to_path_buf(),
            source: Box::new(Error::Io(e)),
        }
    })?;
    Ok(Some(HomePage {
        body,
        path: path.to_path_buf(),
    }))
}

/// The fully-loaded site: every page on disk, plus the cross-cutting
/// indexes built in pass 1 of the rendering pipeline (slug → page,
/// tag → pages, target → pages-that-link-here).
pub struct Site {
    pub config: Config,
    /// Blog posts, sorted newest-first by `created` date.
    pub blog: Vec<Page<BlogFrontmatter>>,
    /// Wiki pages, sorted by filename.
    pub wiki: Vec<Page<WikiFrontmatter>>,
    /// Standalone pages (e.g. About, Contact), sorted by filename.
    pub pages: Vec<Page<PageFrontmatter>>,
    /// Optional home-page content loaded from `<source_dir>/home.md`.
    pub home: Option<HomePage>,
    /// Index of every slug across blog/wiki/pages — supports `[[wiki-link]]`
    /// resolution. Crate-private; use [`Site::get`] to look up.
    pub(crate) slug_index: HashMap<Slug, (PageKind, usize)>,
    /// Tag → slugs of pages with that tag (blog and wiki only; standalone
    /// pages have no tags).
    pub tag_index: HashMap<String, Vec<Slug>>,
    /// Target slug → slugs of pages that link to it via `[[…]]`.
    pub backlinks: HashMap<Slug, Vec<Slug>>,
}

struct SitePages<'a> {
    blog: &'a [Page<BlogFrontmatter>],
    wiki: &'a [Page<WikiFrontmatter>],
    pages: &'a [Page<PageFrontmatter>],
}

impl<'a> SitePages<'a> {
    fn new(
        blog: &'a [Page<BlogFrontmatter>],
        wiki: &'a [Page<WikiFrontmatter>],
        pages: &'a [Page<PageFrontmatter>],
    ) -> Self {
        Self { blog, wiki, pages }
    }

    fn iter_entries(self) -> impl Iterator<Item = (PageKind, usize, &'a Slug, &'a Path)> {
        self.blog
            .iter()
            .enumerate()
            .map(|(index, page)| (PageKind::Blog, index, &page.slug, page.path.as_path()))
            .chain(
                self.wiki
                    .iter()
                    .enumerate()
                    .map(|(index, page)| (PageKind::Wiki, index, &page.slug, page.path.as_path())),
            )
            .chain(
                self.pages
                    .iter()
                    .enumerate()
                    .map(|(index, page)| (PageKind::Page, index, &page.slug, page.path.as_path())),
            )
    }

    fn iter_sources(self) -> impl Iterator<Item = (&'a Slug, &'a str)> {
        self.blog
            .iter()
            .map(|page| (&page.slug, page.body.as_str()))
            .chain(
                self.wiki
                    .iter()
                    .map(|page| (&page.slug, page.body.as_str())),
            )
            .chain(
                self.pages
                    .iter()
                    .map(|page| (&page.slug, page.body.as_str())),
            )
    }

    fn iter_views(self) -> impl Iterator<Item = &'a dyn PageView> {
        self.blog
            .iter()
            .map(|p| p as &dyn PageView)
            .chain(self.wiki.iter().map(|p| p as &dyn PageView))
            .chain(self.pages.iter().map(|p| p as &dyn PageView))
    }
}

fn check_collision(slug: &Slug, path: &Path, seen: &HashMap<Slug, PathBuf>) -> Result<(), Error> {
    if let Some(existing) = seen.get(slug) {
        return Err(Error::SlugCollision {
            slug: slug.to_string(),
            path1: existing.clone(),
            path2: path.to_path_buf(),
        });
    }
    Ok(())
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned()
}

fn load_dir<F, S>(dir: &Path, to_slug: S) -> Result<Vec<Page<F>>, Error>
where
    F: DeserializeOwned,
    S: Fn(&F, &Path) -> String,
{
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;
    entries.retain(|p| p.extension().and_then(|e| e.to_str()) == Some("md"));

    entries.sort();

    let mut pages = Vec::with_capacity(entries.len());
    for path in entries {
        let content = fs::read_to_string(&path).map_err(|e| {
            if e.kind() == io::ErrorKind::InvalidData {
                return Error::NotUtf8 { path: path.clone() };
            }
            Error::LoadPage {
                path: path.clone(),
                source: Box::new(Error::Io(e)),
            }
        })?;
        let (fm, body) = frontmatter::parse::<F>(&content).map_err(|e| Error::LoadPage {
            path: path.clone(),
            source: Box::new(e),
        })?;
        let slug: Slug = to_slug(&fm, &path).into();
        pages.push(Page {
            slug,
            body,
            path,
            frontmatter: fm,
        });
    }

    Ok(pages)
}

impl Site {
    /// Load all content from disk and build a fully-indexed `Site` in one step.
    ///
    /// Pages with `draft: true` in their frontmatter are dropped here, so
    /// they never enter the slug index, tag index, or backlink graph —
    /// wiki-links targeting drafts will fail to resolve as if the file
    /// did not exist on disk.
    pub fn load(config: Config) -> Result<Self, Error> {
        let src = &config.source_dir;
        let mut blog = load_dir(&src.join("blog"), |fm: &BlogFrontmatter, _| fm.slug.clone())?;
        let mut wiki = load_dir(&src.join("wiki"), |_: &WikiFrontmatter, path: &Path| {
            stem(path)
        })?;
        let mut pages = load_dir(&src.join("pages"), |_: &PageFrontmatter, path: &Path| {
            stem(path)
        })?;
        let home = load_home(&src.join("home.md"))?;

        for page in &mut wiki {
            if page.frontmatter.title.is_empty() {
                page.frontmatter.title = page.slug.to_title();
            }
        }

        blog.retain(|p| !p.frontmatter.draft);
        wiki.retain(|p| !p.frontmatter.draft);
        pages.retain(|p| !p.frontmatter.draft);

        blog.sort();

        let mut site = Self::from_parts(config, blog, wiki, pages)?;
        site.home = home;
        Ok(site)
    }

    /// Build a fully-indexed `Site` from pre-loaded page vectors. The optional
    /// home-page content can be set on the returned value's `home` field.
    pub fn from_parts(
        config: Config,
        blog: Vec<Page<BlogFrontmatter>>,
        wiki: Vec<Page<WikiFrontmatter>>,
        pages: Vec<Page<PageFrontmatter>>,
    ) -> Result<Self, Error> {
        let slug_index = Self::build_slug_index(&blog, &wiki, &pages)?;
        let tag_index = Self::build_tag_index(&blog, &wiki);
        let backlinks = Self::build_backlinks(&blog, &wiki, &pages, &slug_index);

        Ok(Self {
            config,
            blog,
            wiki,
            pages,
            home: None,
            slug_index,
            tag_index,
            backlinks,
        })
    }

    /// Map every slug to the kind and position of the page that owns it.
    /// Returns `Error::SlugCollision` if two pages claim the same slug.
    fn build_slug_index(
        blog: &[Page<BlogFrontmatter>],
        wiki: &[Page<WikiFrontmatter>],
        pages: &[Page<PageFrontmatter>],
    ) -> Result<HashMap<Slug, (PageKind, usize)>, Error> {
        let mut slug_index: HashMap<Slug, (PageKind, usize)> = HashMap::new();
        let mut slug_paths: HashMap<Slug, PathBuf> = HashMap::new();
        let site_pages = SitePages::new(blog, wiki, pages);

        for (kind, index, slug, path) in site_pages.iter_entries() {
            check_collision(slug, path, &slug_paths)?;
            slug_index.insert(slug.clone(), (kind, index));
            slug_paths.insert(slug.clone(), path.to_path_buf());
        }

        Ok(slug_index)
    }

    /// Group blog and wiki page slugs by tag. Standalone pages contribute none.
    fn build_tag_index(
        blog: &[Page<BlogFrontmatter>],
        wiki: &[Page<WikiFrontmatter>],
    ) -> HashMap<String, Vec<Slug>> {
        let mut tag_index: HashMap<String, Vec<Slug>> = HashMap::new();
        let tagged = blog
            .iter()
            .map(|p| (&p.slug, p.frontmatter.tags.as_slice()))
            .chain(
                wiki.iter()
                    .map(|p| (&p.slug, p.frontmatter.tags.as_slice())),
            );
        for (slug, tags) in tagged {
            for tag in tags {
                tag_index.entry(tag.clone()).or_default().push(slug.clone());
            }
        }
        tag_index
    }

    /// For each page, find the wiki-links it contains and invert into a
    /// `target -> [source pages]` map. Targets that don't resolve to a known
    /// slug are dropped. Repeated links from one source to one target count
    /// once.
    fn build_backlinks(
        blog: &[Page<BlogFrontmatter>],
        wiki: &[Page<WikiFrontmatter>],
        pages: &[Page<PageFrontmatter>],
        slug_index: &HashMap<Slug, (PageKind, usize)>,
    ) -> HashMap<Slug, Vec<Slug>> {
        let mut backlinks: HashMap<Slug, Vec<Slug>> = HashMap::new();
        let site_pages = SitePages::new(blog, wiki, pages);

        for (from_slug, body) in site_pages.iter_sources() {
            let unique_targets: HashSet<Slug> = extract_wiki_links(body)
                .into_iter()
                .map(|link| link.target.into())
                .filter(|target| slug_index.contains_key(target))
                .collect();
            for target in unique_targets {
                backlinks.entry(target).or_default().push(from_slug.clone());
            }
        }

        backlinks
    }

    pub fn get(&self, slug: &Slug) -> Option<&dyn PageView> {
        match self.slug_index.get(slug)? {
            (PageKind::Blog, idx) => Some(&self.blog[*idx] as &dyn PageView),
            (PageKind::Wiki, idx) => Some(&self.wiki[*idx] as &dyn PageView),
            (PageKind::Page, idx) => Some(&self.pages[*idx] as &dyn PageView),
        }
    }

    pub fn iter_pages(&self) -> impl Iterator<Item = &dyn PageView> {
        SitePages::new(&self.blog, &self.wiki, &self.pages).iter_views()
    }

    pub fn backlinks_for(&self, slug: &Slug) -> Vec<&dyn PageView> {
        self.backlinks
            .get(slug)
            .map(|sources| sources.iter().filter_map(|s| self.get(s)).collect())
            .unwrap_or_default()
    }

    /// Look up a blog post by slug. Returns `None` if the slug doesn't
    /// resolve, or resolves to a non-blog page.
    pub fn blog_post(&self, slug: &Slug) -> Option<&Page<BlogFrontmatter>> {
        match self.slug_index.get(slug)? {
            (PageKind::Blog, idx) => self.blog.get(*idx),
            _ => None,
        }
    }

    /// Look up a wiki page by slug. Returns `None` if the slug doesn't
    /// resolve, or resolves to a non-wiki page.
    pub fn wiki_page(&self, slug: &Slug) -> Option<&Page<WikiFrontmatter>> {
        match self.slug_index.get(slug)? {
            (PageKind::Wiki, idx) => self.wiki.get(*idx),
            _ => None,
        }
    }

    /// Find the posts one step newer and one step older than `page` in the
    /// blog feed. `Site::blog` is sorted newest-first, so the newer
    /// neighbour is at index `i - 1` and the older at `i + 1`.
    pub fn blog_neighbours_of(
        &self,
        page: &Page<BlogFrontmatter>,
    ) -> (
        Option<&Page<BlogFrontmatter>>,
        Option<&Page<BlogFrontmatter>>,
    ) {
        let Some(idx) = self.blog.iter().position(|p| p.slug == page.slug) else {
            return (None, None);
        };
        let newer = idx.checked_sub(1).and_then(|i| self.blog.get(i));
        let older = self.blog.get(idx + 1);
        (newer, older)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use chrono::NaiveDate;

    use super::*;
    use crate::content::frontmatter::{BlogFrontmatter, PageFrontmatter, WikiFrontmatter};
    use crate::content::page::{Page, PageKind, PageView};
    use crate::content::slug::Slug;
    use crate::testutil::write_file;

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

    fn make_wiki_page(slug: &str) -> Page<WikiFrontmatter> {
        let slug: Slug = slug.into();
        Page {
            slug: slug.clone(),
            body: String::new(),
            path: PathBuf::from(format!("content/wiki/{slug}.md")),
            frontmatter: WikiFrontmatter {
                title: slug.to_title(),
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
    fn site_get_returns_correct_variant() {
        let config: Config = "title = \"Test\"\nbase_url = \"https://example.com\""
            .parse()
            .unwrap();

        let mut slug_index = HashMap::new();
        slug_index.insert(Slug::from("post-one"), (PageKind::Blog, 0));
        slug_index.insert(Slug::from("glossary"), (PageKind::Wiki, 0));
        slug_index.insert(Slug::from("about"), (PageKind::Page, 0));

        let site = Site {
            config,
            blog: vec![make_blog_page("post-one")],
            wiki: vec![make_wiki_page("glossary")],
            pages: vec![make_standalone_page("about")],
            home: None,
            slug_index,
            tag_index: HashMap::new(),
            backlinks: HashMap::new(),
        };

        assert_eq!(
            site.get(&Slug::from("post-one")).map(PageView::kind),
            Some(PageKind::Blog)
        );
        assert_eq!(
            site.get(&Slug::from("glossary")).map(PageView::kind),
            Some(PageKind::Wiki)
        );
        assert_eq!(
            site.get(&Slug::from("about")).map(PageView::kind),
            Some(PageKind::Page)
        );
        assert!(site.get(&Slug::from("nonexistent")).is_none());
    }

    fn minimal_config(source_dir: &Path) -> Config {
        format!(
            "title = \"Test\"\nbase_url = \"http://example.com\"\nsource_dir = \"{}\"",
            source_dir.display()
        )
        .parse()
        .unwrap()
    }

    #[test]
    fn happy_path_loads_all_three_kinds() {
        let dir = TempDir::new().unwrap();
        let src = dir.path();

        write_file(
            &src.join("blog/intro.md"),
            "\
---
title: Intro
slug: intro
author: Alice
created: 2024-01-01
---
Hello blog.
",
        );
        write_file(
            &src.join("wiki/glossary.md"),
            "\
---
{}
---
Wiki page.
",
        );
        write_file(
            &src.join("pages/about.md"),
            "\
---
title: About
---
About page.
",
        );

        let cfg = minimal_config(src);
        let site = Site::load(cfg).unwrap();

        assert_eq!(site.blog.len(), 1);
        assert_eq!(site.blog[0].slug.to_string(), "intro");
        assert_eq!(site.wiki.len(), 1);
        assert_eq!(site.wiki[0].slug.to_string(), "glossary");
        assert_eq!(site.pages.len(), 1);
        assert_eq!(site.pages[0].slug.to_string(), "about");
        assert!(site.home.is_none());
    }

    #[test]
    fn home_md_loaded_when_present() {
        let dir = TempDir::new().unwrap();
        let src = dir.path();

        write_file(&src.join("home.md"), "# Welcome\n\nHello from home.md.\n");

        let cfg = minimal_config(src);
        let site = Site::load(cfg).unwrap();

        let home = site.home.as_ref().expect("home.md should be loaded");
        assert!(home.body.contains("# Welcome"));
        assert!(home.body.contains("Hello from home.md."));
    }

    #[test]
    fn home_md_absent_yields_none() {
        let dir = TempDir::new().unwrap();
        let cfg = minimal_config(dir.path());
        let site = Site::load(cfg).unwrap();
        assert!(site.home.is_none());
    }

    #[test]
    fn drafts_are_excluded_from_load() {
        let dir = TempDir::new().unwrap();
        let src = dir.path();

        write_file(
            &src.join("blog/published.md"),
            "\
---
title: Published
slug: published
author: A
created: 2024-01-01
---
",
        );
        write_file(
            &src.join("blog/wip.md"),
            "\
---
title: WIP
slug: wip
author: A
created: 2024-02-01
draft: true
---
",
        );
        write_file(
            &src.join("wiki/draft-page.md"),
            "\
---
draft: true
---
",
        );
        write_file(
            &src.join("pages/secret.md"),
            "\
---
title: Secret
draft: true
---
",
        );

        let site = Site::load(minimal_config(src)).unwrap();

        assert_eq!(site.blog.len(), 1);
        assert_eq!(site.blog[0].slug.to_string(), "published");
        assert!(site.wiki.is_empty());
        assert!(site.pages.is_empty());
        assert!(!site.slug_index.contains_key(&Slug::from("wip")));
        assert!(!site.slug_index.contains_key(&Slug::from("secret")));
    }

    #[test]
    fn missing_pages_dir_returns_empty() {
        let dir = TempDir::new().unwrap();
        let src = dir.path();

        write_file(
            &src.join("blog/post.md"),
            "\
---
title: Post
slug: post
author: Alice
created: 2024-01-01
---
Body.
",
        );

        let cfg = minimal_config(src);
        let site = Site::load(cfg).unwrap();
        assert!(site.pages.is_empty());
    }

    #[test]
    fn blog_missing_required_slug_is_error() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("blog/post.md"),
            "\
---
title: No Slug Post
author: Alice
created: 2024-01-01
---
Body.
",
        );

        let cfg = minimal_config(dir.path());
        let result = Site::load(cfg);
        assert!(matches!(result, Err(Error::LoadPage { .. })));
    }

    #[test]
    fn page_missing_required_title_is_error() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("pages/about.md"),
            "\
---
order: 1
---
Body.
",
        );

        let cfg = minimal_config(dir.path());
        let result = Site::load(cfg);
        assert!(matches!(result, Err(Error::LoadPage { .. })));
    }

    #[test]
    fn pages_are_sorted_by_path() {
        let dir = TempDir::new().unwrap();
        let src = dir.path();

        for slug in ["zebra", "alpha", "mango"] {
            write_file(
                &src.join(format!("wiki/{slug}.md")),
                &format!("---\ntitle: {slug}\n---\nBody.\n"),
            );
        }

        let cfg = minimal_config(src);
        let site = Site::load(cfg).unwrap();
        let slugs: Vec<_> = site.wiki.iter().map(|p| p.slug.to_string()).collect();
        assert_eq!(slugs, ["alpha", "mango", "zebra"]);
    }

    #[test]
    fn non_md_files_are_ignored() {
        let dir = TempDir::new().unwrap();
        let src = dir.path();

        write_file(&src.join("wiki/page.md"), "---\n{}\n---\nBody.\n");
        write_file(&src.join("wiki/readme.txt"), "not a page");
        write_file(&src.join("wiki/.hidden"), "also ignored");

        let cfg = minimal_config(src);
        let site = Site::load(cfg).unwrap();
        assert_eq!(site.wiki.len(), 1);
    }

    #[test]
    fn non_utf8_file_gives_clear_error() {
        let dir = TempDir::new().unwrap();
        let src = dir.path();

        let bad_path = src.join("wiki/bad.md");
        fs::create_dir_all(bad_path.parent().unwrap()).unwrap();
        fs::write(&bad_path, b"\xff\xfe invalid utf8").unwrap();

        let cfg = minimal_config(src);
        let result = Site::load(cfg);
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error for non-UTF-8 file"),
        };
        assert!(err.contains("not valid UTF-8"), "got: {err}");
    }
}
