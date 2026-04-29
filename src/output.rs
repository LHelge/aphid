use std::fs;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::render::{RenderedSite, Theme};

/// Recursively copy all files from `src` into `dest`, preserving directory
/// structure. Skips hidden files (dotfiles). Creates parent dirs as needed.
pub(crate) fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), Error> {
    if !src.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }

        let src_path = entry.path();
        let dest_path = dest.join(&name);

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct OutputWriter {
    dir: PathBuf,
}

impl OutputWriter {
    /// Create a new `OutputWriter`, validating the path and **deleting any
    /// existing contents**. Refuses paths that look unsafe (`.`, empty, `..`
    /// components, root-level, or the current working directory).
    pub fn new(output_dir: &Path) -> Result<Self, Error> {
        let dir = Self::resolve_safe_dir(output_dir)?;
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Write a fully rendered site to disk: every page, the 404 page, and
    /// theme + user static files.
    pub fn write(
        &self,
        site: &RenderedSite,
        theme: &Theme,
        user_static: &Path,
    ) -> Result<(), Error> {
        for (url_path, html) in &site.pages {
            self.write_page(url_path, html)?;
        }
        self.write_404(&site.not_found_html)?;

        tracing::info!("writing root files");
        self.write_root_files(&site.root_files)?;

        tracing::info!("copying static files");
        self.copy_static(theme, user_static)?;
        Ok(())
    }

    /// Write rendered HTML to `{dir}/{url_path}/index.html`. The root path
    /// `"/"` writes to `{dir}/index.html`.
    fn write_page(&self, url_path: &str, html: &str) -> Result<(), Error> {
        let relative = url_path.trim_start_matches('/');

        let unsafe_component = Path::new(relative).components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        });
        if unsafe_component {
            return Err(Error::UnsafeOutputPath {
                path: PathBuf::from(url_path),
                reason: "url path contains unsupported components",
            });
        }

        let dest = self.dir.join(relative).join("index.html");
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, html)?;
        Ok(())
    }

    /// Write the 404 page.
    fn write_404(&self, html: &str) -> Result<(), Error> {
        let dest = self.dir.join("404.html");
        fs::write(&dest, html)?;
        Ok(())
    }

    /// Write generated root files (favicon variants, robots.txt, sitemap.xml)
    /// directly into the output directory.
    fn write_root_files(&self, files: &[(String, Vec<u8>)]) -> Result<(), Error> {
        for (name, bytes) in files {
            let dest = self.dir.join(name);
            fs::write(&dest, bytes)?;
        }
        Ok(())
    }

    /// Copy theme and user static files into the output directory.
    fn copy_static(&self, theme: &Theme, user_static_dir: &Path) -> Result<(), Error> {
        let dest = self.dir.join("static");
        theme.write_static(&dest)?;
        copy_dir_recursive(user_static_dir, &dest)?;
        Ok(())
    }

    /// Validate `output_dir` against the safety rules and resolve it to an
    /// absolute, canonicalised path suitable for `fs::remove_dir_all`.
    fn resolve_safe_dir(output_dir: &Path) -> Result<PathBuf, Error> {
        if output_dir.as_os_str().is_empty() || output_dir == Path::new(".") {
            return Err(Error::UnsafeOutputPath {
                path: output_dir.to_path_buf(),
                reason: "must not be empty or '.'",
            });
        }

        if output_dir
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(Error::UnsafeOutputPath {
                path: output_dir.to_path_buf(),
                reason: "must not contain '..' components",
            });
        }

        let cwd = std::env::current_dir()?;
        let resolved = if output_dir.is_absolute() {
            output_dir.to_path_buf()
        } else {
            cwd.join(output_dir)
        };

        let validated = if resolved.exists() {
            resolved.canonicalize()?
        } else {
            resolved
        };

        if validated.parent().is_none() {
            return Err(Error::UnsafeOutputPath {
                path: validated,
                reason: "must not be a root-level path",
            });
        }

        if let Ok(cwd_canonical) = cwd.canonicalize()
            && validated == cwd_canonical
        {
            return Err(Error::UnsafeOutputPath {
                path: validated,
                reason: "must not be the current working directory",
            });
        }

        Ok(validated)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::testutil::write_file;

    #[test]
    fn user_static_files_copied() {
        let dir = TempDir::new().unwrap();
        let user_static = dir.path().join("static");
        let output = dir.path().join("dist");

        write_file(&user_static.join("css/style.css"), "body {}");
        write_file(&user_static.join("js/app.js"), "console.log('hi')");

        let writer = OutputWriter::new(&output).unwrap();
        let theme = Theme::default();
        writer.copy_static(&theme, &user_static).unwrap();

        assert_eq!(
            fs::read_to_string(output.join("static/css/style.css")).unwrap(),
            "body {}"
        );
        assert_eq!(
            fs::read_to_string(output.join("static/js/app.js")).unwrap(),
            "console.log('hi')"
        );
    }

    #[test]
    fn theme_and_user_files_merged() {
        let dir = TempDir::new().unwrap();
        let theme_dir = dir.path().join("mytheme");
        let user_static = dir.path().join("static");
        let output = dir.path().join("dist");

        write_file(
            &theme_dir.join("theme.toml"),
            "name = \"t\"\nversion = \"0.1.0\"",
        );
        let tmpl_dir = theme_dir.join("templates");
        for name in crate::render::theme::REQUIRED_TEMPLATES {
            write_file(&tmpl_dir.join(name), "{{ content }}");
        }
        write_file(&theme_dir.join("static/css/theme.css"), "theme {}");
        write_file(&theme_dir.join("static/css/shared.css"), "theme-shared");
        let theme = Theme::from_dir(&theme_dir).unwrap();

        write_file(&user_static.join("css/style.css"), "user {}");
        write_file(&user_static.join("css/shared.css"), "user-shared");

        let writer = OutputWriter::new(&output).unwrap();
        writer.copy_static(&theme, &user_static).unwrap();

        assert_eq!(
            fs::read_to_string(output.join("static/css/theme.css")).unwrap(),
            "theme {}"
        );
        assert_eq!(
            fs::read_to_string(output.join("static/css/style.css")).unwrap(),
            "user {}"
        );
        assert_eq!(
            fs::read_to_string(output.join("static/css/shared.css")).unwrap(),
            "user-shared"
        );
    }

    #[test]
    fn missing_static_dirs_not_error() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("dist");
        let nonexistent = dir.path().join("no-such-dir");

        let writer = OutputWriter::new(&output).unwrap();
        let theme = Theme::default();
        writer.copy_static(&theme, &nonexistent).unwrap();
    }

    #[test]
    fn hidden_files_skipped() {
        let dir = TempDir::new().unwrap();
        let user_static = dir.path().join("static");
        let output = dir.path().join("dest");

        write_file(&user_static.join("visible.txt"), "yes");
        write_file(&user_static.join(".hidden"), "no");
        write_file(&user_static.join(".git/config"), "no");

        copy_dir_recursive(&user_static, &output).unwrap();

        assert!(output.join("visible.txt").exists());
        assert!(!output.join(".hidden").exists());
        assert!(!output.join(".git").exists());
    }

    #[test]
    fn write_page_creates_clean_url_path() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("dist");

        let writer = OutputWriter::new(&output).unwrap();
        writer.write_page("/wiki/foo/", "<p>foo</p>").unwrap();
        writer.write_page("/blog/hello/", "<p>hello</p>").unwrap();

        assert_eq!(
            fs::read_to_string(output.join("wiki/foo/index.html")).unwrap(),
            "<p>foo</p>"
        );
        assert_eq!(
            fs::read_to_string(output.join("blog/hello/index.html")).unwrap(),
            "<p>hello</p>"
        );
    }

    #[test]
    fn write_page_standalone_page() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("dist");

        let writer = OutputWriter::new(&output).unwrap();
        writer.write_page("/about/", "<p>about</p>").unwrap();

        assert_eq!(
            fs::read_to_string(output.join("about/index.html")).unwrap(),
            "<p>about</p>"
        );
    }

    #[test]
    fn write_page_root_writes_index_html() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("dist");

        let writer = OutputWriter::new(&output).unwrap();
        writer.write_page("/", "<h1>Blog</h1>").unwrap();

        assert_eq!(
            fs::read_to_string(output.join("index.html")).unwrap(),
            "<h1>Blog</h1>"
        );
    }

    #[test]
    fn new_removes_stale_and_recreates() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("dist");
        fs::create_dir_all(&output).unwrap();
        write_file(&output.join("stale.html"), "old");

        let _writer = OutputWriter::new(&output).unwrap();

        assert!(output.exists());
        assert!(!output.join("stale.html").exists());
    }

    #[test]
    fn new_creates_if_missing() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("dist");

        let _writer = OutputWriter::new(&output).unwrap();

        assert!(output.exists());
    }

    #[test]
    fn new_rejects_dot() {
        let err = OutputWriter::new(Path::new(".")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("refusing"), "expected refusal, got: {msg}");
    }

    #[test]
    fn new_rejects_empty() {
        let err = OutputWriter::new(Path::new("")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("refusing"), "expected refusal, got: {msg}");
    }

    #[test]
    fn new_rejects_parent_dir_components() {
        let err = OutputWriter::new(Path::new("dist/../../etc")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("refusing"), "expected refusal, got: {msg}");
    }

    #[test]
    fn write_page_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("dist");

        let writer = OutputWriter::new(&output).unwrap();
        let err = writer.write_page("/../etc/passwd", "bad").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("refusing"), "expected refusal, got: {msg}");
    }

    #[test]
    fn write_page_rejects_dotdot() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("dist");

        let writer = OutputWriter::new(&output).unwrap();
        let err = writer
            .write_page("/blog/../../../etc/passwd", "bad")
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("refusing"), "expected refusal, got: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_recursive_skips_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        let dest = dir.path().join("dest");
        fs::create_dir_all(&src).unwrap();
        write_file(&src.join("real.txt"), "real");

        let outside = dir.path().join("outside.txt");
        fs::write(&outside, "outside").unwrap();
        symlink(&outside, src.join("link.txt")).unwrap();

        copy_dir_recursive(&src, &dest).unwrap();

        assert!(dest.join("real.txt").exists());
        assert!(!dest.join("link.txt").exists());
    }
}
