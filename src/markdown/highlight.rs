use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
use std::sync::OnceLock;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::{SyntaxDefinition, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::html::escape_html;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

const EXTRA_SYNTAXES: &[(&str, &str)] = &[
    ("TOML", include_str!("syntaxes/toml.sublime-syntax")),
    (
        "TypeScript",
        include_str!("syntaxes/typescript.sublime-syntax"),
    ),
    (
        "Dockerfile",
        include_str!("syntaxes/dockerfile.sublime-syntax"),
    ),
];

fn build_syntax_set() -> SyntaxSet {
    let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
    for (name, source) in EXTRA_SYNTAXES {
        match SyntaxDefinition::load_from_str(source, true, Some(name)) {
            Ok(syn) => builder.add(syn),
            Err(e) => tracing::warn!(syntax = name, "failed to load extra syntax: {e}"),
        }
    }
    builder.build()
}

/// Syntax highlighter for fenced code blocks. Emits CSS class-based
/// markup (`class="hl-…"`) instead of inline styles, so themes control
/// the colour scheme. Wraps `syntect` and a once-initialised `SyntaxSet`
/// containing the syntect defaults plus bundled extras (TOML, TypeScript,
/// Dockerfile).
pub struct Highlighter {
    syntax_set: &'static SyntaxSet,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    pub fn new() -> Self {
        let syntax_set = SYNTAX_SET.get_or_init(build_syntax_set);
        Self { syntax_set }
    }

    pub fn transform<'a>(&self, events: Vec<Event<'a>>) -> Vec<Event<'a>> {
        let mut out = Vec::with_capacity(events.len());
        let mut code_state: Option<(Option<String>, String)> = None;

        for event in events {
            match code_state.take() {
                None => match event {
                    Event::Start(Tag::CodeBlock(kind)) => {
                        let lang = match &kind {
                            CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                                Some(lang.to_string())
                            }
                            _ => None,
                        };
                        code_state = Some((lang, String::new()));
                    }
                    other => out.push(other),
                },
                Some(mut state) => match event {
                    Event::End(TagEnd::CodeBlock) => {
                        let html = self.highlight(&state.0, &state.1);
                        out.push(Event::Html(html.into()));
                    }
                    Event::Text(t) => {
                        state.1.push_str(&t);
                        code_state = Some(state);
                    }
                    other => {
                        code_state = Some(state);
                        out.push(other);
                    }
                },
            }
        }

        out
    }

    fn highlight(&self, lang: &Option<String>, code: &str) -> String {
        let syntax = lang
            .as_deref()
            .and_then(|l| self.syntax_set.find_syntax_by_token(l))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut generator =
            ClassedHTMLGenerator::new_with_class_style(syntax, self.syntax_set, CLASS_STYLE);

        for line in LinesWithEndings::from(code) {
            if generator
                .parse_html_for_line_which_includes_newline(line)
                .is_err()
            {
                return format!(
                    "<pre class=\"code-block\"><code>{}</code></pre>",
                    escape_html(code)
                );
            }
        }

        let inner = generator.finalize();
        format!("<pre class=\"code-block\"><code>{inner}</code></pre>")
    }
}

#[cfg(test)]
mod tests {
    use pulldown_cmark::{Options, Parser};

    use super::*;

    fn render_with_highlight(input: &str) -> String {
        let events: Vec<_> = Parser::new_ext(input, Options::empty()).collect();
        let highlighter = Highlighter::new();
        let events = highlighter.transform(events);
        crate::markdown::render_html(events)
    }

    #[test]
    fn fenced_rust_block_produces_pre_with_classes() {
        let html = render_with_highlight("```rust\nfn main() {}\n```\n");
        assert!(html.contains("<pre class=\"code-block\""));
        assert!(html.contains("class=\"hl-"));
        assert!(html.contains("fn"));
        assert!(html.contains("main"));
        assert!(!html.contains("```"));
    }

    #[test]
    fn unknown_language_falls_back_gracefully() {
        let html = render_with_highlight("```unknownlang\nhello\n```\n");
        assert!(html.contains("<pre class=\"code-block\""));
        assert!(html.contains("hello"));
    }

    #[test]
    fn plain_code_block_rendered() {
        let html = render_with_highlight("```\nsome code\n```\n");
        assert!(html.contains("<pre class=\"code-block\""));
        assert!(html.contains("some code"));
    }

    #[test]
    fn html_escaped_in_plain_fallback() {
        let html = render_with_highlight("```\n<script>alert(1)</script>\n```\n");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn no_inline_styles_in_output() {
        let html = render_with_highlight("```rust\nlet x = 42;\n```\n");
        assert!(
            !html.contains("style=\""),
            "should use CSS classes, not inline styles"
        );
    }
}
