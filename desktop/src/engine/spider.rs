use std::collections::HashSet;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SpiderMatch {
    pub url: String,
    pub page: String,
}

pub const MAX_PAGES: usize = 200;

pub fn crawl(
    start: &str,
    pattern: &str,
    depth: usize,
    max_pages: usize,
) -> Result<Vec<SpiderMatch>, String> {
    let start = normalize_start(start);
    let origin = origin_of(&start).ok_or("start URL needs http:// or https://")?;
    let budget = max_pages.clamp(1, MAX_PAGES);
    let mut seen: HashSet<String> = HashSet::new();
    let mut collected: HashSet<String> = HashSet::new();
    let mut pending: Vec<(String, usize)> = vec![(start, 0)];
    let mut matches: Vec<SpiderMatch> = Vec::new();
    let mut fetched = 0usize;

    while let Some((page_url, level)) = pending.pop() {
        if fetched >= budget {
            break;
        }
        if !seen.insert(page_url.clone()) {
            continue;
        }
        let body = match fetch(&page_url) {
            Ok(body) => body,
            Err(err) => {
                if fetched == 0 {
                    return Err(err);
                }
                continue;
            }
        };
        fetched += 1;

        for href in super::site_explorer::regex_simple(&body) {
            let Some(url) = resolve(&page_url, &href) else {
                continue;
            };
            if origin_of(&url).as_deref() != Some(origin.as_str()) {
                continue;
            }
            let url = clean(&url);
            if is_page(&url) {
                if level < depth && !seen.contains(&url) {
                    pending.push((url, level + 1));
                }
            } else if wants(&url, pattern) && collected.insert(url.clone()) {
                matches.push(SpiderMatch {
                    url,
                    page: page_url.clone(),
                });
            }
        }
    }

    matches.sort_by(|a, b| a.url.cmp(&b.url));
    Ok(matches)
}

fn fetch(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .set("User-Agent", "LotusDM/0.1")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|err| format!("{url}: {err}"))?;
    resp.into_string()
        .map_err(|err| format!("{url}: {err}"))
}

fn normalize_start(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    }
}

fn origin_of(url: &str) -> Option<String> {
    let scheme_end = url.find("://")?;
    let after = &url[scheme_end + 3..];
    let host_end = after.find('/').unwrap_or(after.len());
    if host_end == 0 {
        return None;
    }
    Some(format!("{}{}", &url[..scheme_end + 3], &after[..host_end]))
}

fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("javascript:")
        || lower.starts_with("mailto:")
        || lower.starts_with("data:")
        || lower.starts_with("tel:")
    {
        return None;
    }
    let origin = origin_of(base)?;
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(href.to_string());
    }
    if let Some(rest) = href.strip_prefix("//") {
        let scheme = &base[..base.find("://")?];
        return Some(format!("{}://{}", scheme, rest));
    }
    if href.starts_with('/') {
        return Some(format!("{origin}{href}"));
    }
    let base = base.split('#').next().unwrap_or(base);
    let base = base.split('?').next().unwrap_or(base);
    let dir_end = base.rfind('/')? + 1;
    Some(format!("{}{}", &base[..dir_end], href))
}

fn clean(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let (scheme, rest) = url.split_at(scheme_end + 3);
    let mut segments: Vec<&str> = Vec::new();
    for segment in rest.split('/') {
        match segment {
            "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    format!("{scheme}{}", segments.join("/"))
}

fn path_of(url: &str) -> &str {
    let rest = url.split('#').next().unwrap_or(url);
    rest.split('?').next().unwrap_or(rest)
}

fn is_page(url: &str) -> bool {
    let path = path_of(url);
    let last = path.rsplit('/').next().unwrap_or(path);
    if last.is_empty() {
        return true;
    }
    let Some(dot) = last.rfind('.') else {
        return true;
    };
    let extension = last[dot + 1..].to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "html" | "htm" | "php" | "asp" | "aspx" | "jsp" | "shtml" | "xhtml"
    )
}

fn wants(url: &str, pattern: &str) -> bool {
    if is_page(url) {
        return false;
    }
    let pattern = pattern.trim().to_ascii_lowercase();
    if pattern.is_empty() {
        return true;
    }
    path_of(url).to_ascii_lowercase().contains(&pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_links_resolve_against_the_page() {
        assert_eq!(
            resolve("https://example.com/dir/page.html", "file.zip"),
            Some("https://example.com/dir/file.zip".to_string())
        );
        assert_eq!(
            resolve("https://example.com/dir/page.html", "/abs/file.zip"),
            Some("https://example.com/abs/file.zip".to_string())
        );
        assert_eq!(
            resolve("https://example.com/dir/page.html", "//cdn.example.com/x.zip"),
            Some("https://cdn.example.com/x.zip".to_string())
        );
        assert_eq!(
            resolve("https://example.com/dir/page.html", "sub/"),
            Some("https://example.com/dir/sub/".to_string())
        );
    }

    #[test]
    fn dot_segments_are_cleaned_up() {
        assert_eq!(
            clean("https://example.com/a/b/../c/file.zip"),
            "https://example.com/a/c/file.zip"
        );
        assert_eq!(
            clean("https://example.com/a/./b/file.zip"),
            "https://example.com/a/b/file.zip"
        );
    }

    #[test]
    fn pages_are_told_apart_from_files() {
        assert!(is_page("https://example.com/index.html"));
        assert!(is_page("https://example.com/"));
        assert!(is_page("https://example.com/dir/"));
        assert!(!is_page("https://example.com/file.zip"));
        assert!(!is_page("https://example.com/image.png"));
    }

    #[test]
    fn an_empty_pattern_keeps_files_only() {
        assert!(wants("https://example.com/file.zip", ""));
        assert!(!wants("https://example.com/index.html", ""));
        assert!(wants("https://example.com/file.zip", "zip"));
        assert!(!wants("https://example.com/file.zip", "png"));
    }
}
