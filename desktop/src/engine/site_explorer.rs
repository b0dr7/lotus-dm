#[derive(Clone, Debug)]
pub struct SiteEntry {
    pub name: String,
    pub url: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

pub fn fetch(url: &str) -> Result<Vec<SiteEntry>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", "LotusDM/0.1")
        .call()
        .map_err(|e| format!("fetch: {e}"))?;
    let body = resp.into_string().map_err(|e| format!("read: {e}"))?;
    let base = url.trim_end_matches('/');
    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in regex_simple(&body) {
        let href = cap.trim();
        if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") || href.starts_with("mailto:") {
            continue;
        }
        let full = if href.starts_with("http://") || href.starts_with("https://") {
            href.to_string()
        } else if href.starts_with('/') {
            // resolve against host
            if let Some(scheme_end) = base.find("://") {
                let after = &base[scheme_end + 3..];
                let host_end = after.find('/').unwrap_or(after.len());
                let host = &base[..scheme_end + 3 + host_end];
                format!("{host}{href}")
            } else {
                format!("{base}/{href}")
            }
        } else {
            format!("{base}/{href}")
        };

        if full == base || full == format!("{base}/") {
            continue;
        }
        if !seen.insert(full.clone()) {
            continue;
        }

        let name = href.trim_end_matches('/').rsplit('/').next().unwrap_or(href).to_string();
        if name.is_empty() {
            continue;
        }
        let is_dir = href.ends_with('/');
        entries.push(SiteEntry {
            name,
            url: full,
            is_dir,
            size: None,
        });
    }

    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return if a.is_dir { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
        }
        a.name.cmp(&b.name)
    });

    Ok(entries)
}

/// Every `href` value in the page, in document order.
pub(crate) fn regex_simple(html: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("href=") {
        rest = &rest[start + 5..];
        let (quote, offset) = if rest.starts_with('"') {
            ('"', 1)
        } else if rest.starts_with('\'') {
            ('\'', 1)
        } else {
            // unquoted
            let end = rest.find(|c: char| c.is_whitespace() || c == '>' || c == '<').unwrap_or(rest.len());
            result.push(rest[..end].to_string());
            rest = &rest[end..];
            continue;
        };
        let search = &rest[offset..];
        if let Some(end) = search.find(quote) {
            result.push(search[..end].to_string());
            rest = &search[end + 1..];
        } else {
            break;
        }
    }
    result
}
