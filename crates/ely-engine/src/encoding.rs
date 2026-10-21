//! Reading and writing text files without changing what they mean.

use std::io;
use std::path::Path;
use std::time::SystemTime;

/// How a file's bytes are read and written back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Gb18030,
}

impl Encoding {
    /// The name shown when the file is not plain UTF-8.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf8Bom => "UTF-8 (BOM)",
            Encoding::Utf16Le => "UTF-16 LE",
            Encoding::Gb18030 => "GB18030",
        }
    }
}

/// What the file looked like when it was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

/// A file's text, its encoding and what it looked like.
pub struct Loaded {
    pub text: String,
    pub encoding: Encoding,
    pub fingerprint: Fingerprint,
    /// The file was larger than the window; the text is only the beginning.
    pub truncated: bool,
}

/// How much of a text file is read; a document is not a disk image.
pub const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// The file's size and time, for spotting an outside change.
pub fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(Fingerprint {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

/// Reads a file, keeping the encoding and the byte order mark.
pub fn load(path: &Path) -> Loaded {
    let Ok(bytes) = std::fs::read(path) else {
        return Loaded {
            text: String::new(),
            encoding: Encoding::Utf8,
            fingerprint: Fingerprint {
                len: 0,
                modified: None,
            },
            truncated: false,
        };
    };
    let fingerprint = fingerprint(path).unwrap_or(Fingerprint {
        len: bytes.len() as u64,
        modified: None,
    });
    let (bytes, truncated) = if bytes.len() > MAX_TEXT_BYTES {
        log::warn!(
            "engine: {} is bigger than the window; only the beginning is read",
            path.display()
        );
        (bytes[..MAX_TEXT_BYTES].to_vec(), true)
    } else {
        (bytes, false)
    };
    let (text, encoding) = decode(&bytes);
    Loaded {
        text,
        encoding,
        fingerprint,
        truncated,
    }
}

/// The bytes of a text, in the encoding it was read in.
fn decode(bytes: &[u8]) -> (String, Encoding) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (
            String::from_utf8_lossy(&bytes[3..]).into_owned(),
            Encoding::Utf8Bom,
        );
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let words = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return (String::from_utf16_lossy(&words), Encoding::Utf16Le);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return (text.to_string(), Encoding::Utf8);
    }
    // Chinese text on a Chinese system is usually GB18030.
    let (text, had_errors) = encoding_rs::GB18030.decode_without_bom_handling(bytes);
    if had_errors {
        (String::from_utf8_lossy(bytes).into_owned(), Encoding::Utf8)
    } else {
        (text.into_owned(), Encoding::Gb18030)
    }
}

/// Writes a text in the encoding it was read in, through a temporary file.
pub fn store(path: &Path, text: &str, encoding: Encoding) -> io::Result<()> {
    let bytes = match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => {
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(text.as_bytes());
            bytes
        }
        Encoding::Utf16Le => {
            let mut bytes = vec![0xFF, 0xFE];
            bytes.extend(text.encode_utf16().flat_map(|word| word.to_le_bytes()));
            bytes
        }
        Encoding::Gb18030 => {
            let (bytes, _, _) = encoding_rs::GB18030.encode(text);
            bytes.into_owned()
        }
    };
    let temporary = temporary_for(path);
    std::fs::write(&temporary, &bytes)?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(error)
        }
    }
}

/// A temporary name of its own, beside the target and hidden from the folder.
fn temporary_for(path: &Path) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let count = COUNT.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "document".to_string());
    let folder = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    folder.join(format!(".{name}.{}.{count}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn utf8_round_trips() {
        let path = scratch("mpelyoffice-utf8.txt");
        store(&path, "第一行\n第二行", Encoding::Utf8).expect("the file is written");
        let loaded = load(&path);
        assert_eq!(loaded.text, "第一行\n第二行");
        assert_eq!(loaded.encoding, Encoding::Utf8);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_byte_order_mark_survives_a_round_trip() {
        let path = scratch("mpelyoffice-bom.txt");
        store(&path, "内容", Encoding::Utf8Bom).expect("the file is written");
        let bytes = std::fs::read(&path).expect("the file is read");
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let loaded = load(&path);
        assert_eq!(loaded.text, "内容");
        assert_eq!(loaded.encoding, Encoding::Utf8Bom);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn chinese_text_outside_utf8_is_kept_as_it_was() {
        let path = scratch("mpelyoffice-gb18030.txt");
        let (bytes, _, _) = encoding_rs::GB18030.encode("中文内容");
        std::fs::write(&path, bytes.as_ref()).expect("the file is written");
        let loaded = load(&path);
        assert_eq!(loaded.encoding, Encoding::Gb18030);
        assert_eq!(loaded.text, "中文内容");
        store(&path, &loaded.text, loaded.encoding).expect("the file is written back");
        let again = load(&path);
        assert_eq!(again.text, "中文内容");
        assert_eq!(again.encoding, Encoding::Gb18030);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn utf16_is_kept_as_it_was() {
        let path = scratch("mpelyoffice-utf16.txt");
        store(&path, "宽字符", Encoding::Utf16Le).expect("the file is written");
        let loaded = load(&path);
        assert_eq!(loaded.encoding, Encoding::Utf16Le);
        assert_eq!(loaded.text, "宽字符");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_outside_change_is_seen() {
        let path = scratch("mpelyoffice-fingerprint.txt");
        store(&path, "一", Encoding::Utf8).expect("the file is written");
        let first = fingerprint(&path).expect("the file has a fingerprint");
        store(&path, "一而长", Encoding::Utf8).expect("the file is written");
        let second = fingerprint(&path).expect("the file has a fingerprint");
        assert_ne!(first.len, second.len);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn every_write_uses_a_name_of_its_own() {
        let path = scratch("mpelyoffice-temporary.txt");
        store(&path, "一", Encoding::Utf8).expect("the file is written");
        store(&path, "二", Encoding::Utf8).expect("the file is written");
        let left = std::fs::read_dir(path.parent().expect("the folder"))
            .expect("the folder is read")
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".mpelyoffice-temporary.txt.")
            })
            .count();
        assert_eq!(left, 0, "no temporary file is left behind");
        std::fs::remove_file(&path).ok();
    }
}
