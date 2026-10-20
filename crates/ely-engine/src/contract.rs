//! The rules every engine answers to.
//!
//! An engine that passes these is fit to sit in the one window next to the
//! others. The checks are run by each engine's own tests, so a new engine
//! cannot be added without answering them.

use gpui::App;

use crate::engine::Source;
use crate::EngineDyn;

/// Checks one engine against the contract; the error says what went wrong.
pub fn check(engine: &mut dyn EngineDyn, cx: &mut App) -> anyhow::Result<()> {
    let kind = engine.kind();
    let opened = engine.open(Source::Blank, cx)?;
    let id = opened.id;

    // The document answers about itself.
    let text = engine
        .text(id, cx)
        .ok_or_else(|| anyhow::anyhow!("{}: a new document has no text", kind.id))?;
    if !text.is_empty() {
        anyhow::bail!("{}: a new document is not empty", kind.id);
    }
    if engine.modified(id, cx) {
        anyhow::bail!("{}: a new document is already changed", kind.id);
    }
    if engine.path(id).is_some() {
        anyhow::bail!("{}: a new document has a file", kind.id);
    }

    // The commands are told apart.
    let commands = engine.commands(id);
    let mut ids: Vec<&str> = commands.iter().map(|command| command.id).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    if ids.len() != count {
        anyhow::bail!("{}: two commands share a name", kind.id);
    }
    for command in &commands {
        if !command.enabled && command.reason.is_none() {
            anyhow::bail!(
                "{}: the command {} is blocked without saying why",
                kind.id,
                command.id
            );
        }
    }

    // The document can be written and read back.
    let path = std::env::temp_dir().join(format!("mpelyoffice-contract-{}.txt", kind.id));
    engine.rename(id, path.clone());
    if engine.path(id).as_deref() != Some(path.as_path()) {
        anyhow::bail!("{}: the document did not take the new name", kind.id);
    }
    engine.write(id, &path, cx)?;
    let written = std::fs::read_to_string(&path).unwrap_or_default();
    if written != text {
        anyhow::bail!("{}: the file does not hold the text", kind.id);
    }
    if engine.modified(id, cx) {
        anyhow::bail!("{}: a written document still counts as changed", kind.id);
    }
    std::fs::remove_file(&path).ok();

    // The document lets go.
    engine.close(id);
    if engine.path(id).is_some() {
        anyhow::bail!("{}: a closed document is still there", kind.id);
    }
    Ok(())
}
