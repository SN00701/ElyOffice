use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::BaseDirs;

use crate::AppId;

/// Everything an app stores lives here; uninstall removes only this.
pub fn data_dir(app: AppId) -> Result<PathBuf> {
    Ok(root()?.join(app.slug()))
}

/// The suite's folder: each app's, and the shared settings.
pub(crate) fn root() -> Result<PathBuf> {
    let base = BaseDirs::new().context("no home directory")?;
    Ok(base.data_local_dir().join("MPelyoffice"))
}

/// Omarchy's active theme directory; `ELY_OMARCHY_DIR` overrides it.
pub fn omarchy_current_dir() -> Option<PathBuf> {
    let home = BaseDirs::new()?.home_dir().to_path_buf();
    omarchy_dir(&home, crate::env::value("ELY_OMARCHY_DIR"))
}

/// Omarchy keeps its state under `~/.local/state`, whatever `XDG_STATE_HOME` says.
fn omarchy_dir(home: &Path, over: Option<String>) -> Option<PathBuf> {
    if let Some(dir) = over {
        return Some(PathBuf::from(dir));
    }
    let current = home.join(".local/state/omarchy/current");
    current.is_dir().then_some(current)
}

#[cfg(test)]
mod tests {
    use super::{data_dir, omarchy_dir};
    use crate::AppId;

    #[test]
    fn each_app_has_its_own_directory_under_mpelyoffice() {
        let doc = data_dir(AppId::Doc).unwrap();
        let mail = data_dir(AppId::Mail).unwrap();

        assert!(doc.ends_with("MPelyoffice/ely-doc"), "{}", doc.display());
        assert!(mail.ends_with("MPelyoffice/ely-mail"), "{}", mail.display());
        assert_ne!(doc, mail);
    }

    #[test]
    fn omarchy_is_found_under_home_and_the_override_wins() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(omarchy_dir(home.path(), None), None);

        let current = home.path().join(".local/state/omarchy/current");
        std::fs::create_dir_all(&current).unwrap();
        assert_eq!(omarchy_dir(home.path(), None), Some(current));
        assert_eq!(
            omarchy_dir(home.path(), Some("/elsewhere".into())),
            Some("/elsewhere".into())
        );
    }
}
