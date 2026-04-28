use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{fs, io};

use serde::de::DeserializeOwned;

use super::frontmatter::{self, BlogFrontmatter, PageFrontmatter, WikiFrontmatter};
use super::page::{Page, PageAny, PageKind};
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
    let body = fs::read_to_string(path).map_err(|e| Error::LoadPage {
        path: path.to_path_buf(),
        source: Box::new(Error::Io(e)),
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
        let content = fs::read_to_string(&path).map_err(|e| Error::LoadPage {
            path: path.clone(),
            source: Box::new(Error::Io(e)),
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
    pub fn load(config: Config) -> Result<Self, Error> {
        let src = &config.source_dir;
        let mut blog = load_dir(&src.join("blog"), |fm: &BlogFrontmatter, _| fm.slug.clone())?;
        let wiki = load_dir(&src.join("wiki"), |_, path: &Path| stem(path))?;
        let pages = load_dir(&src.join("pages"), |_, path: &Path| stem(path))?;
        let home = load_home(&src.join("home.md"))?;

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

        let entries = blog
            .iter()
            .enumerate()
            .map(|(i, p)| (PageKind::Blog, i, &p.slug, &p.path))
            .chain(
                wiki.iter()
                    .enumerate()
                    .map(|(i, p)| (PageKind::Wiki, i, &p.slug, &p.path)),
            )
            .chain(
                pages
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (PageKind::Page, i, &p.slug, &p.path)),
            );

        for (kind, i, slug, path) in entries {
            check_collision(slug, path, &slug_paths)?;
            slug_index.insert(slug.clone(), (kind, i));
            slug_paths.insert(slug.clone(), path.clone());
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
        let sources = blog
            .iter()
            .map(|p| (&p.slug, p.body.as_str()))
            .chain(wiki.iter().map(|p| (&p.slug, p.body.as_str())))
            .chain(pages.iter().map(|p| (&p.slug, p.body.as_str())));

        for (from_slug, body) in sources {
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

    pub fn get(&self, slug: &Slug) -> Option<PageAny<'_>> {
        match self.slug_index.get(slug)? {
            (PageKind::Blog, idx) => Some(PageAny::Blog(&self.blog[*idx])),
            (PageKind::Wiki, idx) => Some(PageAny::Wiki(&self.wiki[*idx])),
            (PageKind::Page, idx) => Some(PageAny::Page(&self.pages[*idx])),
        }
    }

    pub fn backlinks_for(&self, slug: &Slug) -> Vec<PageAny<'_>> {
        self.backlinks
            .get(slug)
            .map(|sources| sources.iter().filter_map(|s| self.get(s)).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use chrono::NaiveDate;

    use super::*;
    use crate::content::frontmatter::{BlogFrontmatter, PageFrontmatter, WikiFrontmatter};
    use crate::content::page::{Page, PageAny, PageKind};
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
            },
        }
    }

    fn make_wiki_page(slug: &str) -> Page<WikiFrontmatter> {
        Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/wiki/{slug}.md")),
            frontmatter: WikiFrontmatter {
                title: None,
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

        assert!(matches!(
            site.get(&Slug::from("post-one")),
            Some(PageAny::Blog(_))
        ));
        assert!(matches!(
            site.get(&Slug::from("glossary")),
            Some(PageAny::Wiki(_))
        ));
        assert!(matches!(
            site.get(&Slug::from("about")),
            Some(PageAny::Page(_))
        ));
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
}
