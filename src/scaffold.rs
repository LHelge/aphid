use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::Error;
use crate::config::Config;
use crate::output::OutputWriter;
use crate::render::{Mode, RenderedSite, Theme};

struct Scaffold {
    dir: PathBuf,
    title: String,
}

pub fn new(name: &str) -> Result<(), Error> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        return Err(Error::Scaffold {
            message: format!("directory '{}' already exists", dir.display()),
        });
    }
    fs::create_dir_all(&dir)?;

    let title = title_from_name(dir.file_name().and_then(|n| n.to_str()).unwrap_or(name));
    let scaffold = Scaffold { dir, title };
    scaffold.write_all()?;
    scaffold.build_site()?;

    tracing::info!(name, "created new site");
    println!("\n  To get started:\n");
    println!("    cd {name}");
    println!("    aphid serve\n");
    Ok(())
}

pub fn init(path: &Path) -> Result<(), Error> {
    let dir = path.to_path_buf();
    if dir.join("aphid.toml").exists() {
        return Err(Error::Scaffold {
            message: format!(
                "directory '{}' already contains an aphid.toml",
                dir.display()
            ),
        });
    }
    fs::create_dir_all(&dir)?;

    let title = dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(title_from_name)
        .unwrap_or_else(|| "My Site".to_string());

    let scaffold = Scaffold { dir, title };
    scaffold.write_all()?;
    scaffold.build_site()?;

    tracing::info!(path = %path.display(), "initialized site");
    if path == Path::new(".") {
        println!("\n  To get started:\n");
        println!("    aphid serve\n");
    } else {
        println!("\n  To get started:\n");
        println!("    cd {}", path.display());
        println!("    aphid serve\n");
    }
    Ok(())
}

fn title_from_name(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_file(path: &Path, content: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

impl Scaffold {
    fn write_all(&self) -> Result<(), Error> {
        self.write_config()?;
        self.write_gitignore()?;
        self.write_blog_post()?;
        self.write_wiki_page()?;
        self.write_about_page()?;
        self.write_home()?;
        self.create_static_dir()?;
        Ok(())
    }

    fn write_config(&self) -> Result<(), Error> {
        let content = format!(
            "title = \"{}\"\nbase_url = \"https://example.com\"\n",
            self.title
        );
        write_file(&self.dir.join("aphid.toml"), &content)
    }

    fn write_gitignore(&self) -> Result<(), Error> {
        write_file(&self.dir.join(".gitignore"), "/dist\n")
    }

    fn write_blog_post(&self) -> Result<(), Error> {
        let date = Local::now().format("%Y-%m-%d");
        let filename = format!("{date}_hello-world.md");
        let content = format!(
            "\
---
title: Hello World
slug: hello-world
author: Your Name
created: {date}
description: My first blog post.
tags:
  - hello
---

# Welcome

This is your first blog post. Edit this file or create new `.md` files in the
`content/blog/` directory to add more posts.

See the [[getting-started]] wiki page for more information.
"
        );
        write_file(&self.dir.join("content/blog").join(filename), &content)
    }

    fn write_wiki_page(&self) -> Result<(), Error> {
        let content = "\
---
title: Getting Started
---

# Writing content

Blog posts live in `content/blog/`, wiki pages in `content/wiki/`, and
standalone pages in `content/pages/`.

# Wiki links

Link between any pages with `[[slug]]` syntax. For example, this page can be
linked from anywhere as `[[getting-started]]`.

# Building

Run `aphid serve` to start the development server, or `aphid build` to render
the site into the `dist/` directory.
";
        write_file(&self.dir.join("content/wiki/getting-started.md"), content)
    }

    fn write_about_page(&self) -> Result<(), Error> {
        let content = format!(
            "\
---
title: About
order: 1
---

This is the about page for {title}. Edit `content/pages/about.md` to update it.
",
            title = self.title
        );
        write_file(&self.dir.join("content/pages/about.md"), &content)
    }

    fn write_home(&self) -> Result<(), Error> {
        let content = format!(
            "\
Welcome to **{title}**. This is the home page content.

Edit `content/home.md` to change this text, or delete the file to use the
default home page layout.
",
            title = self.title
        );
        write_file(&self.dir.join("content/home.md"), &content)
    }

    fn create_static_dir(&self) -> Result<(), Error> {
        fs::create_dir_all(self.dir.join("static"))?;
        Ok(())
    }

    fn build_site(&self) -> Result<(), Error> {
        let config_path = self.dir.join("aphid.toml");
        let output_dir = self.dir.join("dist");
        let config = Config::from_path(&config_path)?;
        let theme = Theme::load(&config)?;
        let rendered = RenderedSite::build(&config, &theme, Mode::Build)?;
        let writer = OutputWriter::new(&output_dir)?;
        writer.write(&rendered, &theme, &config.static_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_from_hyphenated_name() {
        assert_eq!(title_from_name("my-cool-blog"), "My Cool Blog");
    }

    #[test]
    fn title_from_underscored_name() {
        assert_eq!(title_from_name("my_site"), "My Site");
    }

    #[test]
    fn title_from_plain_name() {
        assert_eq!(title_from_name("mysite"), "Mysite");
    }

    #[test]
    fn title_from_mixed_separators() {
        assert_eq!(title_from_name("my-cool_blog"), "My Cool Blog");
    }

    #[test]
    fn new_creates_complete_site() {
        let tmp = tempfile::tempdir().unwrap();
        let site_dir = tmp.path().join("test-site");

        new(site_dir.to_str().unwrap()).unwrap();

        assert!(site_dir.join("aphid.toml").exists());
        assert!(site_dir.join(".gitignore").exists());
        assert!(site_dir.join("content/pages/about.md").exists());
        assert!(site_dir.join("content/wiki/getting-started.md").exists());
        assert!(site_dir.join("content/home.md").exists());
        assert!(site_dir.join("static").is_dir());

        let blog_entries: Vec<_> = fs::read_dir(site_dir.join("content/blog"))
            .unwrap()
            .collect();
        assert_eq!(blog_entries.len(), 1);

        let config = fs::read_to_string(site_dir.join("aphid.toml")).unwrap();
        assert!(config.contains("title = \"Test Site\""));
    }

    #[test]
    fn new_fails_if_directory_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let site_dir = tmp.path().join("existing");
        fs::create_dir(&site_dir).unwrap();

        let err = new(site_dir.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn init_creates_in_existing_directory() {
        let tmp = tempfile::tempdir().unwrap();

        init(tmp.path()).unwrap();

        assert!(tmp.path().join("aphid.toml").exists());
        assert!(tmp.path().join(".gitignore").exists());
        assert!(tmp.path().join("content/blog").is_dir());
    }

    #[test]
    fn init_fails_if_config_exists() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("aphid.toml"), "").unwrap();

        let err = init(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("already contains an aphid.toml"));
    }

    #[test]
    fn init_creates_directory_if_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("nested/site");

        init(&nested).unwrap();

        assert!(nested.join("aphid.toml").exists());
    }
}
