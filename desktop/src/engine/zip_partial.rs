use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::Control;

const EOCD_SIG: u32 = 0x0605_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;
const EOCD_LEN: usize = 22;
const EOCD_SCAN: u64 = 65_557;
const CHUNK: u64 = 1 << 20;
const USER_AGENT: &str = "LotusDM/0.1";

#[derive(Clone, Debug)]
pub struct ZipEntry {
    pub name: String,
    pub method: u16,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub header_offset: u64,
}

impl ZipEntry {
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }

    pub fn is_stored(&self) -> bool {
        self.method == 0
    }
}

pub fn list(url: &str) -> Result<Vec<ZipEntry>, String> {
    let size = content_length(url)?;
    if size < EOCD_LEN as u64 {
        return Err("not a zip (too small)".to_string());
    }
    let scan_start = size.saturating_sub(EOCD_SCAN);
    let tail = fetch_range(url, scan_start, size - 1)?;
    let (cd_size, cd_offset) = find_eocd(&tail)?;
    if cd_size == 0 {
        return Err("the archive has no entries".to_string());
    }
    if cd_offset.saturating_add(cd_size) > size {
        return Err("the central directory points past the end of the file".to_string());
    }
    if cd_size > 32u64 << 20 {
        return Err("the central directory is too large".to_string());
    }
    let directory = fetch_range(url, cd_offset, cd_offset + cd_size - 1)?;
    let entries = parse_central_directory(&directory);
    if entries.is_empty() {
        return Err("no entries in the central directory".to_string());
    }
    Ok(entries)
}

pub fn extract(
    url: &str,
    entry: &ZipEntry,
    folder: &Path,
    control: &Control,
) -> Result<PathBuf, String> {
    if entry.is_dir() {
        return Err("that is a folder".to_string());
    }
    if !entry.is_stored() {
        return Err(format!(
            "entry uses compression method {} (only stored entries can be pulled out)",
            entry.method
        ));
    }
    if entry.compressed_size == 0 {
        return Err("the entry is empty".to_string());
    }
    let header = fetch_range(url, entry.header_offset, entry.header_offset + 29)?;
    if read_u32(&header, 0) != Some(LOCAL_SIG) {
        return Err("entry has no local header".to_string());
    }
    let name_len = read_u16(&header, 26).ok_or("truncated local header")? as u64;
    let extra_len = read_u16(&header, 28).ok_or("truncated local header")? as u64;
    let start = entry.header_offset + 30 + name_len + extra_len;
    let end = start + entry.compressed_size - 1;

    std::fs::create_dir_all(folder).map_err(|err| format!("{folder:?}: {err}"))?;
    let name = safe_name(&entry.name)?;
    let target = folder.join(&name);
    let part = folder.join(format!("{name}.part"));
    let mut file = std::fs::File::create(&part).map_err(|err| format!("{part:?}: {err}"))?;
    let mut offset = start;
    let mut written = 0u64;
    while offset <= end {
        if control.cancelled() {
            let _ = std::fs::remove_file(&part);
            return Err("cancelled".to_string());
        }
        let stop = offset.saturating_add(CHUNK - 1).min(end);
        let bytes = fetch_range(url, offset, stop)?;
        if bytes.is_empty() {
            break;
        }
        file.write_all(&bytes)
            .map_err(|err| format!("{part:?}: {err}"))?;
        written += bytes.len() as u64;
        offset = stop + 1;
    }
    drop(file);
    if written != entry.compressed_size || written != entry.uncompressed_size {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "short read: {written} of {} bytes",
            entry.compressed_size
        ));
    }
    if target.exists() {
        let _ = std::fs::remove_file(&target);
    }
    std::fs::rename(&part, &target).map_err(|err| format!("{target:?}: {err}"))?;
    Ok(target)
}

fn content_length(url: &str) -> Result<u64, String> {
    let resp = ureq::head(url)
        .set("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|err| format!("{url}: {err}"))?;
    resp.header("Content-Length")
        .ok_or_else(|| format!("{url}: the server did not report a size"))?
        .parse::<u64>()
        .map_err(|_| format!("{url}: unreadable size"))
}

fn fetch_range(url: &str, start: u64, end: u64) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Range", &format!("bytes={start}-{end}"))
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|err| format!("{url}: {err}"))?;
    if resp.status() != 206 {
        return Err(format!(
            "{url}: the server ignored the Range request ({})",
            resp.status()
        ));
    }
    let cap = (end - start + 1 + 1024) as usize;
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(cap as u64)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("{url}: {err}"))?;
    Ok(bytes)
}

fn find_eocd(tail: &[u8]) -> Result<(u64, u64), String> {
    if tail.len() < EOCD_LEN {
        return Err("not a zip (no end of archive record)".to_string());
    }
    let last = tail.len() - EOCD_LEN;
    for at in (0..=last).rev() {
        if read_u32(tail, at) != Some(EOCD_SIG) {
            continue;
        }
        let comment = read_u16(tail, at + 20).unwrap_or(0) as usize;
        if at + EOCD_LEN + comment != tail.len() {
            continue;
        }
        let cd_size = read_u32(tail, at + 12).ok_or("truncated end of archive record")? as u64;
        let cd_offset = read_u32(tail, at + 16).ok_or("truncated end of archive record")? as u64;
        if cd_size == u32::MAX as u64 || cd_offset == u32::MAX as u64 {
            return Err("zip64 archives are not supported".to_string());
        }
        return Ok((cd_size, cd_offset));
    }
    Err("not a zip (no end of archive record)".to_string())
}

fn parse_central_directory(bytes: &[u8]) -> Vec<ZipEntry> {
    let mut entries = Vec::new();
    let mut at = 0usize;
    while at + 46 <= bytes.len() {
        if read_u32(bytes, at) != Some(CENTRAL_SIG) {
            break;
        }
        let method = read_u16(bytes, at + 10).unwrap_or(0);
        let compressed_size = read_u32(bytes, at + 20).unwrap_or(0) as u64;
        let uncompressed_size = read_u32(bytes, at + 24).unwrap_or(0) as u64;
        let name_len = read_u16(bytes, at + 28).unwrap_or(0) as usize;
        let extra_len = read_u16(bytes, at + 30).unwrap_or(0) as usize;
        let comment_len = read_u16(bytes, at + 32).unwrap_or(0) as usize;
        let header_offset = read_u32(bytes, at + 42).unwrap_or(0) as u64;
        let name_start = at + 46;
        let name_end = name_start + name_len;
        if name_end > bytes.len() {
            break;
        }
        entries.push(ZipEntry {
            name: String::from_utf8_lossy(&bytes[name_start..name_end]).to_string(),
            method,
            compressed_size,
            uncompressed_size,
            header_offset,
        });
        at = name_end + extra_len + comment_len;
    }
    entries
}

fn safe_name(name: &str) -> Result<String, String> {
    let base = name.rsplit('/').next().unwrap_or(name).trim();
    if base.is_empty() || base == "." || base == ".." || base.contains('\\') || base.contains(':') {
        return Err(format!("unsafe entry name: {name}"));
    }
    Ok(base.to_string())
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    let slice = bytes.get(at..at + 2)?;
    Some(u16::from_le_bytes(slice.try_into().ok()?))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end_record(cd_size: u32, cd_offset: u32, comment: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&EOCD_SIG.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&cd_size.to_le_bytes());
        bytes.extend_from_slice(&cd_offset.to_le_bytes());
        bytes.extend_from_slice(&(comment.len() as u16).to_le_bytes());
        bytes.extend_from_slice(comment);
        bytes
    }

    fn central_record(name: &str, method: u16, size: u32, offset: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&CENTRAL_SIG.to_le_bytes());
        bytes.extend_from_slice(&20u16.to_le_bytes());
        bytes.extend_from_slice(&20u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&method.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes
    }

    #[test]
    fn finds_the_end_record_behind_a_comment() {
        let bytes = end_record(2048, 512, b"signed");
        assert_eq!(find_eocd(&bytes), Ok((2048, 512)));
    }

    #[test]
    fn rejects_zip64_end_records() {
        let bytes = end_record(u32::MAX, 512, b"");
        assert!(find_eocd(&bytes).is_err());
    }

    #[test]
    fn reads_entries_out_of_the_central_directory() {
        let mut bytes = central_record("a/one.bin", 0, 120, 0);
        bytes.extend_from_slice(&central_record("b/two.txt", 8, 40, 4096));
        let entries = parse_central_directory(&bytes);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "a/one.bin");
        assert!(entries[0].is_stored());
        assert_eq!(entries[1].header_offset, 4096);
        assert!(!entries[1].is_stored());
    }

    #[test]
    fn entry_names_cannot_escape_the_folder() {
        assert_eq!(safe_name("dir/file.zip").unwrap(), "file.zip");
        assert_eq!(safe_name("file.zip").unwrap(), "file.zip");
        assert!(safe_name("../evil.zip").is_err());
        assert!(safe_name("dir/").is_err());
        assert!(safe_name("c:evil.zip").is_err());
    }
}
