//! Write AI-agent instruction files for an aphid site.
//!
//! Each supported agent has its own layout convention (filenames, locations,
//! frontmatter shape). The shared instruction text lives in `templates/` and
//! is embedded into the binary via [`include_str!`]; per-tool wrappers in
//! [`claude`], [`copilot`], and [`codex`] adapt that text to the target
//! tool's conventions.

use std::fs;
use std::path::Path;

use crate::Error;

mod claude;
mod codex;
mod copilot;

/// Target agent for [`init`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum AgentTool {
    /// Claude Code — `CLAUDE.md` + `.claude/skills/`.
    Claude,
    /// GitHub Copilot — `.github/copilot-instructions.md` + `.github/instructions/`.
    Copilot,
    /// `AGENTS.md` fallback — Codex, Aider, Goose, and current Cursor all read it.
    Codex,
}

impl AgentTool {
    /// Short label used in the post-init "to get started" footer.
    pub fn label(self) -> &'static str {
        match self {
            AgentTool::Claude => "Claude Code",
            AgentTool::Copilot => "GitHub Copilot",
            AgentTool::Codex => "AGENTS.md (Codex / Aider / Goose / Cursor)",
        }
    }
}

/// Write the instruction files for `tool` into `dir`.
///
/// Skill files are overwritten on every run — they're pure embedded reference
/// content and should track the installed aphid version. The main instruction
/// file (`CLAUDE.md`, `.github/copilot-instructions.md`, `AGENTS.md`) is
/// preserved if it already exists, since users typically extend it with
/// project-specific guidance; a warning is emitted in that case.
pub fn init(tool: AgentTool, dir: &Path) -> Result<(), Error> {
    match tool {
        AgentTool::Claude => claude::write(dir),
        AgentTool::Copilot => copilot::write(dir),
        AgentTool::Codex => codex::write(dir),
    }
}

pub(crate) const MAIN_INSTRUCTIONS: &str = include_str!("templates/main_instructions.md");
pub(crate) const CONTENT_SKILL: &str = include_str!("templates/content_skill.md");
pub(crate) const THEME_SKILL: &str = include_str!("templates/theme_skill.md");

pub(crate) const CONTENT_DESCRIPTION: &str = "Reference for authoring aphid content. Use when writing or editing markdown files under content/blog/, content/wiki/, or content/pages/, or when configuring frontmatter or aphid.toml.";
pub(crate) const THEME_DESCRIPTION: &str = "Reference for editing aphid themes. Use when modifying Tera templates under theme/templates/, designing layouts, working with template variables, or changing theme CSS and static assets.";

/// Write `content` to `path`, creating parent directories as needed.
/// Overwrites any existing file at that path.
pub(crate) fn write_file(path: &Path, content: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

/// Write `content` to `path` only if no file exists there. If one already
/// does, leave it untouched and emit a warning — used for the main
/// instruction file, which users are expected to extend with project-specific
/// guidance.
pub(crate) fn write_main_file(path: &Path, content: &str) -> Result<(), Error> {
    if path.exists() {
        tracing::warn!(
            path = %path.display(),
            "left existing main instruction file untouched; delete it to regenerate"
        );
        return Ok(());
    }
    write_file(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_writes_expected_files() {
        let tmp = tempfile::tempdir().unwrap();
        init(AgentTool::Claude, tmp.path()).unwrap();

        let main = tmp.path().join("CLAUDE.md");
        let content_skill = tmp.path().join(".claude/skills/aphid-content/SKILL.md");
        let theme_skill = tmp.path().join(".claude/skills/aphid-theme/SKILL.md");

        assert!(main.exists());
        assert!(content_skill.exists());
        assert!(theme_skill.exists());

        let body = fs::read_to_string(&content_skill).unwrap();
        assert!(body.starts_with("---\nname: aphid-content\n"));
        assert!(body.contains("Wiki-links"));

        let theme_body = fs::read_to_string(&theme_skill).unwrap();
        assert!(theme_body.starts_with("---\nname: aphid-theme\n"));
        assert!(theme_body.contains("Tera"));
    }

    #[test]
    fn copilot_writes_expected_files() {
        let tmp = tempfile::tempdir().unwrap();
        init(AgentTool::Copilot, tmp.path()).unwrap();

        let main = tmp.path().join(".github/copilot-instructions.md");
        let content = tmp
            .path()
            .join(".github/instructions/aphid-content.instructions.md");
        let theme = tmp
            .path()
            .join(".github/instructions/aphid-theme.instructions.md");

        assert!(main.exists());
        assert!(content.exists());
        assert!(theme.exists());

        let content_body = fs::read_to_string(&content).unwrap();
        assert!(content_body.starts_with("---\napplyTo: \"content/**\"\n---"));

        let theme_body = fs::read_to_string(&theme).unwrap();
        assert!(theme_body.starts_with("---\napplyTo: \"theme/**\"\n---"));
    }

    #[test]
    fn codex_writes_expected_files() {
        let tmp = tempfile::tempdir().unwrap();
        init(AgentTool::Codex, tmp.path()).unwrap();

        let main = tmp.path().join("AGENTS.md");
        let content = tmp.path().join(".agents/aphid-content.md");
        let theme = tmp.path().join(".agents/aphid-theme.md");

        assert!(main.exists());
        assert!(content.exists());
        assert!(theme.exists());

        let main_body = fs::read_to_string(&main).unwrap();
        assert!(main_body.contains(".agents/aphid-content.md"));
        assert!(main_body.contains(".agents/aphid-theme.md"));

        let content_body = fs::read_to_string(&content).unwrap();
        assert!(!content_body.starts_with("---"));
    }

    #[test]
    fn main_file_preserved_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let user_main = "# my own CLAUDE.md\n\nProject-specific guidance.";
        fs::write(tmp.path().join("CLAUDE.md"), user_main).unwrap();

        init(AgentTool::Claude, tmp.path()).unwrap();

        let body = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert_eq!(body, user_main, "main file must not be overwritten");
        assert!(
            tmp.path()
                .join(".claude/skills/aphid-content/SKILL.md")
                .exists(),
            "skill files should still be written"
        );
    }

    #[test]
    fn skill_files_are_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_path = tmp.path().join(".claude/skills/aphid-content/SKILL.md");
        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, "stale").unwrap();

        init(AgentTool::Claude, tmp.path()).unwrap();

        let body = fs::read_to_string(&skill_path).unwrap();
        assert_ne!(body, "stale");
        assert!(body.contains("aphid-content"));
    }

    #[test]
    fn re_running_keeps_main_file_and_refreshes_skills() {
        let tmp = tempfile::tempdir().unwrap();
        init(AgentTool::Codex, tmp.path()).unwrap();
        let main_before = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        fs::write(tmp.path().join(".agents/aphid-content.md"), "stale").unwrap();

        init(AgentTool::Codex, tmp.path()).unwrap();

        let main_after = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        let skill_after = fs::read_to_string(tmp.path().join(".agents/aphid-content.md")).unwrap();
        assert_eq!(main_before, main_after);
        assert_ne!(skill_after, "stale");
    }
}
