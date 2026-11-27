//! Torrent intake: turns whatever the queue holds into something librqbit can
//! add.
//!
//! Three shapes are accepted, all expressed as the job's `url` field:
//! - `magnet:?xt=...` links,
//! - `http(s)://.../file.torrent` links (librqbit downloads the metadata),
//! - absolute paths to a local `.torrent` file.

use std::path::{Path, PathBuf};

use librqbit::AddTorrent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TorrentSource {
    Magnet(String),
    Url(String),
    File(PathBuf),
}

/// Recognise a torrent job URL. Returns `None` for plain http(s) files, which
/// the HTTP engine keeps handling.
pub fn parse(raw: &str) -> Option<TorrentSource> {
    let text = raw.trim();
    if text.is_empty() || text.len() > 8192 {
        return None;
    }

    if is_magnet(text) {
        return Some(TorrentSource::Magnet(text.to_string()));
    }

    if let Some(url) = http_torrent_url(text) {
        return Some(TorrentSource::Url(url.to_string()));
    }

    let path = Path::new(text);
    if has_torrent_extension(path) && path.is_file() {
        return Some(TorrentSource::File(path.to_path_buf()));
    }

    None
}

fn is_magnet(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.starts_with("magnet:?") && lower.contains("xt=urn:")
}

/// http(s) URL whose path (ignoring query and fragment) ends in `.torrent`.
fn http_torrent_url(text: &str) -> Option<&str> {
    let lower = text.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return None;
    }
    let path = text.split(['?', '#']).next().unwrap_or(text);
    has_torrent_extension(Path::new(path)).then_some(text)
}

fn has_torrent_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
}

impl TorrentSource {
    /// Name to show before the metadata (and its real name) arrives.
    pub fn display_name(&self) -> String {
        match self {
            TorrentSource::Magnet(_) => "magnet link".to_string(),
            TorrentSource::Url(url) => crate::core::queue::derive_filename(url),
            TorrentSource::File(path) => path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "torrent".to_string()),
        }
    }

    /// Build the librqbit payload. Local files are read into memory because
    /// librqbit takes either a URL or the raw `.torrent` bytes.
    pub fn to_add(&self) -> Result<AddTorrent<'static>, String> {
        match self {
            TorrentSource::Magnet(magnet) => Ok(AddTorrent::from_url(magnet.clone())),
            TorrentSource::Url(url) => Ok(AddTorrent::from_url(url.clone())),
            TorrentSource::File(path) => {
                let bytes = std::fs::read(path)
                    .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
                if bytes.is_empty() {
                    return Err(format!("{} is empty", path.display()));
                }
                Ok(AddTorrent::from_bytes(bytes))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_magnets() {
        assert!(matches!(
            parse("  magnet:?xt=urn:btih:0123456789abcdef&dn=thing  "),
            Some(TorrentSource::Magnet(_))
        ));
        assert!(parse("magnet:?dn=no-infohash").is_none());
    }

    #[test]
    fn recognises_torrent_links() {
        assert!(matches!(
            parse("https://example.com/files/ubuntu.torrent"),
            Some(TorrentSource::Url(_))
        ));
        assert!(matches!(
            parse("https://example.com/ubuntu.TORRENT?token=1"),
            Some(TorrentSource::Url(_))
        ));
        assert!(parse("https://example.com/ubuntu.iso").is_none());
        assert!(parse("https://example.com/a.torrent/extra").is_none());
    }

    #[test]
    fn ignores_missing_local_files() {
        assert!(parse("C:/definitely/not/here.torrent").is_none());
        assert!(parse("").is_none());
        assert!(parse("ftp://example.com/x.torrent").is_none());
    }
}
