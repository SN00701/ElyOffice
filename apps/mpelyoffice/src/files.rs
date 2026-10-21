//! The files the start page shows: the list, and what is said of each file.
//!
//! The page itself is in `home.rs`; this is what it shows.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ely_engine::{EngineKind, Registry};

/// A file the page shows.
#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub kind: EngineKind,
    /// Where the file is.
    pub place: String,
    /// Who made it; the local user, since there is no cloud here.
    pub author: String,
    /// When it was last changed.
    pub changed: String,
    /// How large it is.
    pub size: String,
}

/// Reads a list of files and describes them.
pub fn read_list(path: &Path, registry: &Registry) -> Vec<Entry> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(paths) = serde_json::from_slice::<Vec<PathBuf>>(&bytes) else {
        return Vec::new();
    };
    paths.iter().map(|path| describe(path, registry)).collect()
}

/// What the page shows of one file.
fn describe(path: &Path, registry: &Registry) -> Entry {
    let metadata = std::fs::metadata(path).ok();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let kind = registry.kind_for(path).unwrap_or(EngineKind {
        id: "unknown",
        extensions: &[],
        default_extension: "",
    });
    let place = path
        .parent()
        .map(|parent| parent.display().to_string())
        .unwrap_or_default();
    let changed = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .map(show_time)
        .unwrap_or_else(|| "—".to_string());
    let size = metadata
        .as_ref()
        .map(|metadata| show_size(metadata.len()))
        .unwrap_or_else(|| "—".to_string());
    Entry {
        path: path.to_path_buf(),
        name,
        kind,
        place,
        author: ely_platform::user::named("").unwrap_or_default(),
        changed,
        size,
    }
}

/// A time, as a date and a clock.
fn show_time(time: SystemTime) -> String {
    let since = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = since / 86_400;
    let seconds = since % 86_400;
    format!(
        "{}-{:02}-{:02} {:02}:{:02}",
        days / 365 + 1970,
        (days % 365) / 30 + 1,
        days % 30 + 1,
        seconds / 3600,
        (seconds % 3600) / 60
    )
}

/// A size, as a reader reads it.
fn show_size(bytes: u64) -> String {
    const UNIT: u64 = 1024;
    if bytes < UNIT {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = "KB";
    for next in ["MB", "GB", "TB"] {
        size /= UNIT as f64;
        unit = next;
        if size < UNIT as f64 {
            break;
        }
    }
    format!("{size:.1} {unit}")
}

/// The folders of the local places.
pub fn local_places() -> Vec<(String, PathBuf)> {
    let mut places = Vec::new();
    if let Ok(home) = std::env::var("USERPROFILE") {
        let home = PathBuf::from(home);
        for (name, folder) in [
            ("桌面", home.join("Desktop")),
            ("文档", home.join("Documents")),
            ("下载", home.join("Downloads")),
        ] {
            if folder.is_dir() {
                places.push((name.to_string(), folder));
            }
        }
    }
    places
}

/// Whether a file was changed in the last week.
pub fn is_recent(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(changed) = metadata.modified() else {
        return false;
    };
    let week = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_secs().saturating_sub(7 * 86_400))
        .unwrap_or(0);
    changed
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_secs() >= week)
        .unwrap_or(false)
}
