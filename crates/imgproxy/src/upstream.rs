use std::time::Duration;

use anyhow::{Result, bail};
use axum::http::StatusCode;

/// Booth's own resize variants; anything else 403s upstream anyway. "orig" = original upload.
const SIZES: &[&str] = &["48x48", "72x72_a2_g5", "300x300_a2_g5", "620x620", "orig"];
const MAX_UPSTREAM_BYTES: usize = 25 * 1024 * 1024;

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.char_indices().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() && !c.is_ascii_uppercase() })
}

fn is_booth_file(s: &str) -> bool {
    let Some((stem, ext)) = s.rsplit_once('.') else { return false };
    (1..=120).contains(&stem.len())
        && stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && ["jpg", "jpeg", "png", "gif"].contains(&ext.to_ascii_lowercase().as_str())
}

/// Upstream URL for a validated request, or None if the path isn't Booth-shaped.
pub(crate) fn upstream_url(size: &str, uuid: &str, item: &str, file: &str) -> Option<String> {
    if !SIZES.contains(&size) || !is_uuid(uuid) || item.is_empty() || item.len() > 12 || !item.bytes().all(|b| b.is_ascii_digit()) || !is_booth_file(file) {
        return None;
    }
    let prefix = if size == "orig" { String::new() } else { format!("c/{size}/") };
    Some(format!("https://booth.pximg.net/{prefix}{uuid}/i/{item}/{file}"))
}

/// Upstream URL for a shop icon; Booth names the file after a UUID, so a new icon is a new address.
pub(crate) fn icon_upstream_url(size: &str, user: &str, file: &str) -> Option<String> {
    if !SIZES.contains(&size) || user.is_empty() || user.len() > 12 || !user.bytes().all(|b| b.is_ascii_digit()) || !is_booth_file(file) {
        return None;
    }
    let prefix = if size == "orig" { String::new() } else { format!("c/{size}/") };
    Some(format!("https://booth.pximg.net/{prefix}users/{user}/icon_image/{file}"))
}

pub(crate) async fn fetch_upstream(client: &reqwest::Client, url: &str) -> Result<Option<Vec<u8>>> {
    // one retry: transient connect errors were seen right after startup
    let mut resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => {
            tokio::time::sleep(Duration::from_millis(300)).await;
            // Debug keeps reqwest's source chain (TLS/DNS/connect detail) that Display drops
            client.get(url).send().await.map_err(|e| anyhow::anyhow!("{e:?}"))?
        }
    };
    // only 404 is cached as permanent; a 403 could be a transient block (e.g. a future Referer check) and must not stick
    if resp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        bail!("upstream {} for {url}", resp.status());
    }
    if resp.content_length().is_some_and(|n| n as usize > MAX_UPSTREAM_BYTES) {
        bail!("upstream too large: {url}");
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_UPSTREAM_BYTES {
            bail!("upstream too large: {url}");
        }
    }
    Ok(Some(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_booth_shaped_paths_pass() {
        let uuid = "dc33a5b5-f8f7-418f-8087-393ea802b496";
        assert_eq!(
            upstream_url("300x300_a2_g5", uuid, "9064622", "7da36066-7fd0_base_resized.jpg").as_deref(),
            Some("https://booth.pximg.net/c/300x300_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/7da36066-7fd0_base_resized.jpg")
        );
        assert_eq!(upstream_url("orig", uuid, "1", "a.png").as_deref(), Some("https://booth.pximg.net/dc33a5b5-f8f7-418f-8087-393ea802b496/i/1/a.png"));
        assert!(upstream_url("999x999", uuid, "1", "a.jpg").is_none()); // unknown size
        assert!(upstream_url("orig", "not-a-uuid", "1", "a.jpg").is_none());
        assert!(upstream_url("orig", uuid, "1a", "a.jpg").is_none());
        assert!(upstream_url("orig", uuid, "1", "../../etc.jpg").is_none());
        assert!(upstream_url("orig", uuid, "1", "a.svg").is_none());
        assert!(upstream_url("orig", uuid, "1", "a.jpg?x=@evil.com").is_none());
        assert_eq!(
            icon_upstream_url("48x48", "15037164", "5fa34b7c-9f3a-4123-889f-ab80a8f3a79d_base_resized.jpg").as_deref(),
            Some("https://booth.pximg.net/c/48x48/users/15037164/icon_image/5fa34b7c-9f3a-4123-889f-ab80a8f3a79d_base_resized.jpg")
        );
        assert!(icon_upstream_url("48x48", "1x", "a.jpg").is_none());
        assert!(icon_upstream_url("999x999", "1", "a.jpg").is_none());
        assert!(icon_upstream_url("48x48", "1", "../a.jpg").is_none());
    }
}
