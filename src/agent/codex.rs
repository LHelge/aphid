use std::path::Path;

use crate::Error;

use super::{CONTENT_SKILL, MAIN_INSTRUCTIONS, THEME_SKILL, write_file, write_main_file};

const CODEX_POINTER: &str = "\n# Detailed references for this project\n\n\
Topic-specific guidance lives in `.agents/`:\n\n\
- `.agents/aphid-content.md` — authoring content (markdown, frontmatter, wiki-links).\n\
- `.agents/aphid-theme.md` — editing themes (Tera templates, variables, assets).\n\n\
Load the relevant one before working on those areas.\n";

pub(super) fn write(dir: &Path) -> Result<(), Error> {
    let main = format!("{MAIN_INSTRUCTIONS}{CODEX_POINTER}");
    write_main_file(&dir.join("AGENTS.md"), &main)?;
    write_file(&dir.join(".agents/aphid-content.md"), CONTENT_SKILL)?;
    write_file(&dir.join(".agents/aphid-theme.md"), THEME_SKILL)?;
    Ok(())
}
