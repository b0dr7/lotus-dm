
use std::path::{Path, PathBuf};

use super::rqbitapi::AddTorrent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TorrentSource {
    Link(String),
    File(PathBuf),
}

pub fn parse(raw: &str) -> Option<TorrentSource> {
    let text = raw.trim();
    if text.is_empty() || text.len() > 8192 {
        return None;
    }

    let lower = text.to_ascii_lowercase();
    if lower.starts_with("magnet:?") && lower.contains("xt=urn:") {
        return Some(TorrentSource::Link(text.to_string()));
    }

    let is_http = lower.starts_with("http:
    let url_path = text.split(['?', '#']).next().unwrap_or(text);
    if is_http && has_torrent_extension(url_path) {
        return Some(TorrentSource::Link(text.to_string()));
    }

    let path = Path::new(text);
    if has_torrent_extension(text) && path.is_file() {
        return Some(TorrentSource::File(path.to_path_buf()));
    }
    None
}

fn has_torrent_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
}

impl TorrentSource {
    pub fn to_add(&self) -> Result<AddTorrent<'static>, String> {
        match self {
            TorrentSource::Link(url) => Ok(AddTorrent::from_url(url.clone())),
            TorrentSource::File(path) => {
                let bytes = std::fs::read(path)
                    .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
                Ok(AddTorrent::from_bytes(bytes))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_magnets_and_torrent_links() {
        for url in [
            "  magnet:?xt=urn:btih:0123456789abcdef&dn=thing  ",
            "https:
            "https:
        ] {
            assert!(matches!(parse(url), Some(TorrentSource::Link(_))), "{url}");
        }
    }

    #[test]
    fn leaves_everything_else_to_the_http_engine() {
        for url in [
            "",
            "magnet:?dn=no-infohash",
            "https:
            "https:
            "ftp:
            "C:/definitely/not/here.torrent",
        ] {
            assert!(parse(url).is_none(), "{url}");
        }
    }
}
