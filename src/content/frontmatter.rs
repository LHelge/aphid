use chrono::NaiveDate;
use pulldown_cmark::{Event, MetadataBlockKind, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, de::DeserializeOwned};

use crate::Error;

/// Frontmatter for a blog post. `slug` is required and authoritative —
/// the filename stem is ignored for blog posts so authors can rename files
/// without breaking permalinks.
#[derive(Debug, Deserialize)]
pub struct BlogFrontmatter {
    pub title: String,
    pub slug: String,
    pub author: String,
    pub created: NaiveDate,
    pub updated: Option<NaiveDate>,
    pub image: Option<String>,
    /// Short summary shown in blog listings (home page, blog index).
    /// When omitted, listing layouts simply skip the description paragraph.
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Frontmatter for a wiki page. Title is optional — when omitted, the
/// page derives its title from the filename stem (`battery-pack` →
/// "Battery Pack"). Category is optional — when present, the wiki index
/// groups pages under category headings.
#[derive(Debug, Deserialize)]
pub struct WikiFrontmatter {
    pub title: Option<String>,
    pub category: Option<String>,
    pub created: Option<NaiveDate>,
    pub updated: Option<NaiveDate>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Frontmatter for a standalone page (About, Contact, etc.). `order`
/// controls placement in the site nav: lower values come first; pages
/// without an `order` are sorted alphabetically at the end.
#[derive(Debug, Deserialize)]
pub struct PageFrontmatter {
    pub title: String,
    pub order: Option<i32>,
}

/// Extract YAML frontmatter and markdown body from a source string.
///
/// Returns `(frontmatter, body)` where `body` is the content after the
/// closing `---` delimiter, with leading whitespace stripped.
pub fn parse<F: DeserializeOwned>(input: &str) -> Result<(F, String), Error> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

    let mut yaml = String::new();
    let mut body_start: Option<usize> = None;
    let mut in_block = false;

    for (event, range) in Parser::new_ext(input, opts).into_offset_iter() {
        match event {
            Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                in_block = true;
            }
            Event::Text(ref text) if in_block => {
                yaml.push_str(text);
            }
            Event::End(TagEnd::MetadataBlock(_)) => {
                body_start = Some(range.end);
                break;
            }
            _ => {
                if !in_block {
                    break;
                }
            }
        }
    }

    let start = body_start.ok_or(Error::MissingFrontmatter)?;
    let frontmatter = serde_yml::from_str::<F>(&yaml)?;
    let body = input[start..].trim_start().to_string();

    Ok((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn parse_valid_blog_frontmatter() {
        let input = "\
---
title: My Post
slug: my-post
author: Alice
created: 2024-01-15
---

# Hello

Body text here.
";
        let (fm, body): (BlogFrontmatter, String) = parse(input).unwrap();
        assert_eq!(fm.title, "My Post");
        assert_eq!(fm.slug, "my-post");
        assert_eq!(fm.author, "Alice");
        assert_eq!(fm.created, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        assert!(fm.tags.is_empty());
        assert!(body.starts_with("# Hello"));
    }

    #[test]
    fn parse_blog_frontmatter_with_tags() {
        let input = "\
---
title: Tagged
slug: tagged
author: Bob
created: 2024-06-01
tags:
  - rust
  - cli
---

Body.
";
        let (fm, _): (BlogFrontmatter, String) = parse(input).unwrap();
        assert_eq!(fm.tags, ["rust", "cli"]);
    }

    #[test]
    fn parse_empty_frontmatter_block() {
        // WikiFrontmatter has all-optional fields; `{}` is a valid empty YAML map.
        // pulldown-cmark requires at least one character inside the delimiters.
        let input = "---\n{}\n---\n\nSome body.\n";
        let (fm, body): (WikiFrontmatter, String) = parse(input).unwrap();
        assert!(fm.title.is_none());
        assert!(fm.tags.is_empty());
        assert!(body.contains("Some body"));
    }

    #[test]
    fn parse_missing_frontmatter_is_error() {
        let input = "# No frontmatter\n\nJust a heading.\n";
        let result: Result<(WikiFrontmatter, String), _> = parse(input);
        assert!(matches!(result, Err(Error::MissingFrontmatter)));
    }

    #[test]
    fn parse_malformed_yaml_is_error() {
        let input = "---\n: invalid: : yaml\n---\n\nBody\n";
        let result: Result<(WikiFrontmatter, String), _> = parse(input);
        assert!(matches!(result, Err(Error::FrontmatterParse(_))));
    }
}
