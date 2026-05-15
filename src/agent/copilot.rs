use std::path::Path;

use crate::Error;

use super::{CONTENT_SKILL, MAIN_INSTRUCTIONS, THEME_SKILL, write_file};

pub(super) fn write(dir: &Path) -> Result<(), Error> {
    write_file(
        &dir.join(".github/copilot-instructions.md"),
        MAIN_INSTRUCTIONS,
    )?;
    write_file(
        &dir.join(".github/instructions/aphid-content.instructions.md"),
        &instruction_file("content/**", CONTENT_SKILL),
    )?;
    write_file(
        &dir.join(".github/instructions/aphid-theme.instructions.md"),
        &instruction_file("theme/**", THEME_SKILL),
    )?;
    Ok(())
}

fn instruction_file(apply_to: &str, body: &str) -> String {
    format!("---\napplyTo: \"{apply_to}\"\n---\n\n{body}")
}
