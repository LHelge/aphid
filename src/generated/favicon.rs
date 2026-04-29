use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};

use crate::Error;

/// A loaded favicon source image, ready to be encoded at any size.
///
/// Owns both the decoded image and the original path (used for error
/// reporting). The encoding methods live here so that image manipulation
/// logic is co-located with its data.
struct Favicon {
    image: DynamicImage,
    path: PathBuf,
}

impl Favicon {
    /// Load a favicon source from disk. SVG files are rasterised at 512 px
    /// via `resvg`; everything else goes through `image::open`.
    fn load(path: &Path) -> Result<Self, Error> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        tracing::info!(source = %path.display(), format = %ext, "loading favicon");
        let image = match ext.as_str() {
            "svg" => Self::load_svg(path)?,
            _ => image::open(path).map_err(|e| Error::Favicon {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?,
        };
        Ok(Self {
            image,
            path: path.to_path_buf(),
        })
    }

    /// Resize to a square of `size` px and encode as PNG.
    fn encode_png(&self, size: u32) -> Result<Vec<u8>, Error> {
        let resized = self.image.resize_to_fill(size, size, FilterType::Lanczos3);
        let mut buf = Cursor::new(Vec::new());
        resized
            .write_to(&mut buf, ImageFormat::Png)
            .map_err(|e| Error::Favicon {
                path: self.path.clone(),
                reason: format!("failed to encode {size}px PNG: {e}"),
            })?;
        Ok(buf.into_inner())
    }

    /// Build a multi-resolution ICO containing 16 px and 32 px images.
    fn encode_ico(&self) -> Result<Vec<u8>, Error> {
        let make_frame = |size: u32| -> Result<IcoFrame<'_>, Error> {
            let img = self.image.resize_to_fill(size, size, FilterType::Lanczos3);
            let rgba = img.into_rgba8();
            IcoFrame::as_png(rgba.as_raw(), size, size, image::ColorType::Rgba8.into()).map_err(
                |e| Error::Favicon {
                    path: self.path.clone(),
                    reason: format!("failed to create {size}px ICO frame: {e}"),
                },
            )
        };

        let frame_16 = make_frame(16)?;
        let frame_32 = make_frame(32)?;

        let mut buf = Vec::new();
        let encoder = IcoEncoder::new(&mut buf);
        encoder
            .encode_images(&[frame_16, frame_32])
            .map_err(|e| Error::Favicon {
                path: self.path.clone(),
                reason: format!("failed to encode ICO: {e}"),
            })?;
        Ok(buf)
    }

    /// Rasterise an SVG at 512 px (largest favicon size we need) so the
    /// resulting `DynamicImage` can be down-scaled like any other raster.
    fn load_svg(path: &Path) -> Result<DynamicImage, Error> {
        let svg_data = std::fs::read_to_string(path).map_err(|e| Error::Favicon {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

        let tree = resvg::usvg::Tree::from_str(&svg_data, &resvg::usvg::Options::default())
            .map_err(|e| Error::Favicon {
                path: path.to_path_buf(),
                reason: format!("failed to parse SVG: {e}"),
            })?;

        let svg_size = tree.size();
        let target = 512_f32;
        let scale = target / svg_size.width().max(svg_size.height());
        let width = (svg_size.width() * scale).round() as u32;
        let height = (svg_size.height() * scale).round() as u32;

        let mut pixmap =
            resvg::tiny_skia::Pixmap::new(width, height).ok_or_else(|| Error::Favicon {
                path: path.to_path_buf(),
                reason: "failed to create pixmap".into(),
            })?;

        let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let png_data = pixmap.encode_png().map_err(|e| Error::Favicon {
            path: path.to_path_buf(),
            reason: format!("failed to encode rasterised SVG: {e}"),
        })?;

        image::load_from_memory_with_format(&png_data, ImageFormat::Png).map_err(|e| {
            Error::Favicon {
                path: path.to_path_buf(),
                reason: format!("failed to decode rasterised SVG: {e}"),
            }
        })
    }
}

/// The set of favicon files generated from a single source image, plus the
/// HTML `<link>` tags to inject into every page's `<head>`.
#[derive(Clone)]
pub struct FaviconSet {
    /// `(filename, bytes)` — files to write at the site root.
    pub files: Vec<(String, Vec<u8>)>,
    /// HTML fragment containing all `<link>` tags for the generated favicons.
    pub html_tags: String,
}

const FAVICON_HTML: &str = concat!(
    "    <link rel=\"icon\" href=\"/favicon.ico\" sizes=\"32x32\">\n",
    "    <link rel=\"icon\" type=\"image/png\" sizes=\"192x192\" href=\"/android-chrome-192x192.png\">\n",
    "    <link rel=\"icon\" type=\"image/png\" sizes=\"512x512\" href=\"/android-chrome-512x512.png\">\n",
    "    <link rel=\"apple-touch-icon\" href=\"/apple-touch-icon.png\">\n",
    "    <link rel=\"manifest\" href=\"/site.webmanifest\">\n",
);

impl FaviconSet {
    /// Load the source image (raster or SVG), resize it to every standard
    /// favicon size, and produce the corresponding files and HTML tags.
    ///
    /// The four image-encoding steps (3 PNGs + 1 ICO) run in parallel via
    /// rayon since they are independent CPU-bound operations on the same
    /// immutable source image.
    pub fn generate(source: &Path, site_title: &str) -> Result<Self, Error> {
        let favicon = Favicon::load(source)?;

        tracing::info!("encoding favicon sizes (parallel)");
        // Parallel encode: each closure captures &favicon (immutable).
        let ((apple, android_192), (android_512, ico)) = rayon::join(
            || rayon::join(|| favicon.encode_png(180), || favicon.encode_png(192)),
            || rayon::join(|| favicon.encode_png(512), || favicon.encode_ico()),
        );

        let files = vec![
            ("apple-touch-icon.png".into(), apple?),
            ("android-chrome-192x192.png".into(), android_192?),
            ("android-chrome-512x512.png".into(), android_512?),
            ("favicon.ico".into(), ico?),
            (
                "site.webmanifest".into(),
                Self::webmanifest(site_title).into_bytes(),
            ),
        ];

        Ok(Self {
            files,
            html_tags: FAVICON_HTML.to_string(),
        })
    }

    fn webmanifest(site_title: &str) -> String {
        let escaped = site_title.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            "{{\"name\":\"{}\",\"icons\":[{{\"src\":\"/android-chrome-192x192.png\",\"sizes\":\"192x192\",\"type\":\"image/png\"}},{{\"src\":\"/android-chrome-512x512.png\",\"sizes\":\"512x512\",\"type\":\"image/png\"}}]}}",
            escaped
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    fn test_png_bytes() -> Vec<u8> {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            4,
            4,
            image::Rgba([255, 0, 0, 255]),
        ));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn generate_from_raster() {
        let dir = TempDir::new().unwrap();
        let icon_path = dir.path().join("icon.png");
        std::fs::write(&icon_path, test_png_bytes()).unwrap();

        let set = FaviconSet::generate(&icon_path, "Test Site").unwrap();

        let names: Vec<&str> = set.files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"favicon.ico"));
        assert!(names.contains(&"apple-touch-icon.png"));
        assert!(names.contains(&"android-chrome-192x192.png"));
        assert!(names.contains(&"android-chrome-512x512.png"));
        assert!(names.contains(&"site.webmanifest"));
        assert_eq!(set.files.len(), 5);

        assert!(set.html_tags.contains("favicon.ico"));
        assert!(set.html_tags.contains("apple-touch-icon"));
        assert!(set.html_tags.contains("site.webmanifest"));
    }

    #[test]
    fn generate_from_svg() {
        let dir = TempDir::new().unwrap();
        let svg_path = dir.path().join("icon.svg");
        std::fs::write(
            &svg_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" fill="red"/></svg>"#,
        )
        .unwrap();

        let set = FaviconSet::generate(&svg_path, "SVG Site").unwrap();
        assert_eq!(set.files.len(), 5);
    }

    #[test]
    fn webmanifest_contains_title() {
        let json = FaviconSet::webmanifest("My Cool Site");
        assert!(json.contains("My Cool Site"));
        assert!(json.contains("android-chrome-192x192.png"));
        assert!(json.contains("android-chrome-512x512.png"));
    }

    #[test]
    fn webmanifest_escapes_special_chars() {
        let json = FaviconSet::webmanifest(r#"Site "with" quotes"#);
        assert!(json.contains(r#"Site \"with\" quotes"#));
    }
}
