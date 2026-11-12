use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use flate2::{Compression, write::GzEncoder};
use rayon::prelude::*;

use crate::Error;

/// Writes brotli (`.br`) and gzip (`.gz`) siblings next to the text files of
/// a built site, for servers that can send a precompressed variant instead of
/// compressing on every request (`aphid-server` does).
///
/// The originals are kept: they serve clients that accept no encoding, and
/// hosts that ignore the variants.
pub(crate) struct Precompressor;

/// How many files were considered and how many variants were written.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PrecompressStats {
    pub files: usize,
    pub variants: usize,
}

impl Precompressor {
    /// Extensions worth compressing. Images, fonts and other already
    /// compressed formats are left alone.
    const EXTENSIONS: &[&str] = &[
        "html",
        "css",
        "js",
        "mjs",
        "json",
        "xml",
        "svg",
        "txt",
        "map",
        "webmanifest",
    ];
    /// Below this size the saving is not worth two more files.
    const MIN_SIZE: u64 = 1024;
    /// Maximum quality: this runs once per build, not once per request.
    const BROTLI_QUALITY: u32 = 11;
    /// Maximum quality is slow on big inputs (seconds per megabyte), so large
    /// files such as vendored script bundles get a faster level that gives
    /// up about a tenth of the saving.
    const BROTLI_LARGE_QUALITY: u32 = 9;
    const BROTLI_LARGE_FILE: usize = 512 * 1024;
    const BROTLI_WINDOW: u32 = 22;
    const BROTLI_BUFFER: usize = 4096;

    /// Compress every eligible file under `dir`, in parallel.
    pub(crate) fn run(dir: &Path) -> Result<PrecompressStats, Error> {
        let mut files = Vec::new();
        Self::collect(dir, &mut files)?;

        let variants = files
            .par_iter()
            .map(|path| Self::compress(path))
            .try_reduce(|| 0, |a, b| Ok(a + b))?;

        Ok(PrecompressStats {
            files: files.len(),
            variants,
        })
    }

    fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                Self::collect(&path, files)?;
            } else if file_type.is_file()
                && Self::is_compressible(&path)
                && entry.metadata()?.len() >= Self::MIN_SIZE
            {
                files.push(path);
            }
        }
        Ok(())
    }

    fn is_compressible(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                Self::EXTENSIONS
                    .iter()
                    .any(|known| ext.eq_ignore_ascii_case(known))
            })
    }

    /// Write the variants of one file; returns how many were kept.
    fn compress(path: &Path) -> Result<usize, Error> {
        let original = fs::read(path)?;
        let variants = [
            ("br", Self::brotli(&original)?),
            ("gz", Self::gzip(&original)?),
        ];

        let mut written = 0;
        for (suffix, compressed) in variants {
            // A variant that is not smaller would only cost the client time.
            if compressed.len() < original.len() {
                File::create(Self::variant_path(path, suffix))?.write_all(&compressed)?;
                written += 1;
            }
        }
        Ok(written)
    }

    /// `index.html` → `index.html.br`.
    fn variant_path(path: &Path, suffix: &str) -> PathBuf {
        let mut name = path.as_os_str().to_owned();
        name.push(".");
        name.push(suffix);
        PathBuf::from(name)
    }

    fn brotli(input: &[u8]) -> io::Result<Vec<u8>> {
        let quality = if input.len() >= Self::BROTLI_LARGE_FILE {
            Self::BROTLI_LARGE_QUALITY
        } else {
            Self::BROTLI_QUALITY
        };
        let mut encoder = brotli::CompressorWriter::new(
            Vec::new(),
            Self::BROTLI_BUFFER,
            quality,
            Self::BROTLI_WINDOW,
        );
        encoder.write_all(input)?;
        encoder.flush()?;
        Ok(encoder.into_inner())
    }

    fn gzip(input: &[u8]) -> io::Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(input)?;
        encoder.finish()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use tempfile::TempDir;

    use super::*;
    use crate::testutil::write_file;

    fn page() -> String {
        "<p>the quick brown fox jumps over the lazy dog</p>\n".repeat(100)
    }

    #[test]
    fn writes_variants_that_decode_to_the_original() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("wiki/foo/index.html"), &page());

        let stats = Precompressor::run(dir.path()).unwrap();
        assert_eq!(
            stats,
            PrecompressStats {
                files: 1,
                variants: 2
            }
        );

        let original = dir.path().join("wiki/foo/index.html");
        assert_eq!(fs::read_to_string(&original).unwrap(), page());

        let gz = fs::read(dir.path().join("wiki/foo/index.html.gz")).unwrap();
        let mut decoded = String::new();
        flate2::read::GzDecoder::new(gz.as_slice())
            .read_to_string(&mut decoded)
            .unwrap();
        assert_eq!(decoded, page());

        let br = fs::read(dir.path().join("wiki/foo/index.html.br")).unwrap();
        let mut decoded = String::new();
        brotli::Decompressor::new(br.as_slice(), 4096)
            .read_to_string(&mut decoded)
            .unwrap();
        assert_eq!(decoded, page());
        assert!(br.len() < page().len() / 2);
    }

    #[test]
    fn skips_small_files_and_other_formats() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("tiny.html"), "<p>hi</p>");
        write_file(&dir.path().join("static/photo.png"), &page());
        write_file(&dir.path().join("static/font.woff2"), &page());

        let stats = Precompressor::run(dir.path()).unwrap();
        assert_eq!(stats, PrecompressStats::default());

        let mut names: Vec<_> = fs::read_dir(dir.path().join("static"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["font.woff2", "photo.png"]);
        assert!(!dir.path().join("tiny.html.br").exists());
    }

    #[test]
    fn covers_static_text_assets() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("static/style.CSS"),
            &"body { margin: 0 }\n".repeat(100),
        );
        write_file(
            &dir.path().join("feed.xml"),
            &"<entry>post</entry>\n".repeat(100),
        );

        let stats = Precompressor::run(dir.path()).unwrap();
        assert_eq!(stats.files, 2);
        assert!(dir.path().join("static/style.CSS.br").exists());
        assert!(dir.path().join("feed.xml.gz").exists());
    }
}
