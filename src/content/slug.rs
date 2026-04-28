use std::fmt;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Slug(String);

impl From<&str> for Slug {
    fn from(text: &str) -> Self {
        let mut slug = String::new();
        let mut last_hyphen = true;
        for c in text.chars() {
            if c.is_alphanumeric() {
                slug.extend(c.to_lowercase());
                last_hyphen = false;
            } else if !last_hyphen {
                slug.push('-');
                last_hyphen = true;
            }
        }
        if slug.ends_with('-') {
            slug.pop();
        }
        Self(slug)
    }
}

impl From<String> for Slug {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for Slug {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl Slug {
    pub(crate) fn new_raw(s: String) -> Self {
        Self(s)
    }

    pub fn to_title(&self) -> String {
        self.0
            .split('-')
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_lowercase() {
        assert_eq!(Slug::from("Rust").to_string(), "rust");
    }

    #[test]
    fn spaces_become_hyphens() {
        assert_eq!(Slug::from("my tag").to_string(), "my-tag");
    }

    #[test]
    fn collapses_runs() {
        assert_eq!(Slug::from("a  b  c").to_string(), "a-b-c");
    }

    #[test]
    fn strips_special_chars() {
        assert_eq!(Slug::from("C++").to_string(), "c");
        assert_eq!(Slug::from("hello, world!").to_string(), "hello-world");
    }

    #[test]
    fn preserves_non_ascii() {
        assert_eq!(Slug::from("café").to_string(), "café");
        assert_eq!(Slug::from("résumé").to_string(), "résumé");
    }

    #[test]
    fn trims_hyphens() {
        assert_eq!(Slug::from("!hello!").to_string(), "hello");
    }

    #[test]
    fn already_slug() {
        assert_eq!(Slug::from("my-tag").to_string(), "my-tag");
    }

    #[test]
    fn empty() {
        assert_eq!(Slug::from("").to_string(), "");
    }

    #[test]
    fn to_title_hyphenated() {
        assert_eq!(Slug::from("battery-pack").to_title(), "Battery Pack");
    }

    #[test]
    fn to_title_single_word() {
        assert_eq!(Slug::from("glossary").to_title(), "Glossary");
    }

    #[test]
    fn to_title_multi_word() {
        assert_eq!(Slug::from("my-wiki-page").to_title(), "My Wiki Page");
    }

    #[test]
    fn to_title_empty() {
        assert_eq!(Slug::from("").to_title(), "");
    }
}
