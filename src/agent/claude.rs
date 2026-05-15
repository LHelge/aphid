use std::path::Path;

use crate::Error;

use super::{
    CONTENT_DESCRIPTION, CONTENT_SKILL, MAIN_INSTRUCTIONS, THEME_DESCRIPTION, THEME_SKILL,
    write_file, write_main_file,
};

pub(super) fn write(dir: &Path) -> Result<(), Error> {
    write_main_file(&dir.join("CLAUDE.md"), MAIN_INSTRUCTIONS)?;
    write_file(
        &dir.join(".claude/skills/aphid-content/SKILL.md"),
        &skill_file("aphid-content", CONTENT_DESCRIPTION, CONTENT_SKILL),
    )?;
    write_file(
        &dir.join(".claude/skills/aphid-theme/SKILL.md"),
        &skill_file("aphid-theme", THEME_DESCRIPTION, THEME_SKILL),
    )?;
    Ok(())
}

fn skill_file(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}")
}
