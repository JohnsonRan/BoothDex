use std::time::Duration;

use anyhow::{Result, bail};
use axum::http::StatusCode;
use reqwest::Url;
use reqwest::redirect::Policy;

use crate::public_dns::PublicOnly;

/// Booth's own resize variants; anything else 403s upstream anyway. "orig" = original upload.
pub(crate) const SIZES: &[&str] = &["48x48", "72x72_a2_g5", "300x300_a2_g5", "620x620", "orig"];
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

/// Upstream URL for one of BOOTH's static images, such as the icon of a shop that has none.
pub(crate) fn asset_upstream_url(file: &str) -> Option<String> {
    is_booth_file(file).then(|| format!("https://asset.booth.pm/assets/{file}"))
}

/// Upstream URL for a pixiv Factory drawing that Booth lists as an item image. Every one seen is a png, and the
/// extension is compared exactly because `is_booth_file` accepts any case, which the site never sends.
pub(crate) fn factory_upstream_url(uuid: &str, file: &str) -> Option<String> {
    let png = file.rsplit_once('.').is_some_and(|(_, ext)| ext == "png");
    (png && is_uuid(uuid) && is_booth_file(file)).then(|| format!("https://factory.pixiv.net/files/uploads/i/conceptual_drawing/{uuid}/{file}"))
}

/// A redirect may move along the path of the same https host and port, never to another host, port or scheme: the
/// hosts are fixed by the URL builders above, and a redirect must not lead past them.
fn redirect_allowed(from: &Url, to: &Url) -> bool {
    to.scheme() == "https" && to.host_str() == from.host_str() && to.port_or_known_default() == from.port_or_known_default()
}

/// Redirects followed for one request, reqwest's default count.
const MAX_REDIRECTS: usize = 10;

/// Whether to follow a redirect to `to`, given the URLs already requested: the first is the original request, so
/// `previous` holds one URL more than the redirects followed so far.
fn follow_redirect(previous: &[Url], to: &Url) -> bool {
    previous.len() <= MAX_REDIRECTS && redirect_allowed(&previous[0], to)
}

/// The one client for every upstream host: it follows only the redirects `follow_redirect` accepts, and connects only
/// to the public addresses `PublicOnly` resolves. A refused redirect comes back as the 3xx response, which
/// `fetch_upstream` fails.
pub(crate) fn upstream_client() -> reqwest::Result<reqwest::Client> {
    // reqwest is built with `rustls-no-provider`, so building a client panics until a provider is installed
    rustls::crypto::ring::default_provider().install_default().ok();
    let policy = Policy::custom(|a| if follow_redirect(a.previous(), a.url()) { a.follow() } else { a.stop() });
    reqwest::Client::builder().timeout(Duration::from_secs(20)).redirect(policy).dns_resolver(PublicOnly).build()
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

    #[test]
    fn only_booth_asset_files_pass() {
        let placeholder = format!("thumbnail_placeholder_f_150x150-{}.png", "0".repeat(64));
        assert_eq!(asset_upstream_url(&placeholder), Some(format!("https://asset.booth.pm/assets/{placeholder}")));
        for bad in ["", "../a.png", "a.svg", "a.png?x=@evil.com", "a.b.png", &format!("{}.png", "a".repeat(121))] {
            assert!(asset_upstream_url(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn only_factory_drawings_pass() {
        let uuid = "04cee5a2-f70c-49f2-b9b6-4c09a6b9bb74";
        assert_eq!(
            factory_upstream_url(uuid, "canvas_94032c5836.png").as_deref(),
            Some("https://factory.pixiv.net/files/uploads/i/conceptual_drawing/04cee5a2-f70c-49f2-b9b6-4c09a6b9bb74/canvas_94032c5836.png")
        );
        assert!(factory_upstream_url(&uuid.to_uppercase(), "a.png").is_none());
        assert!(factory_upstream_url("04cee5a2-f70c-49f2-b9b6", "a.png").is_none());
        for bad in ["", "a.jpg", "a.PNG", "../a.png", "a.svg", "a.png?x=@evil.com"] {
            assert!(factory_upstream_url(uuid, bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn redirects_stay_on_the_same_https_host() {
        let url = |s: &str| Url::parse(s).unwrap();
        for host in ["booth.pximg.net", "asset.booth.pm", "factory.pixiv.net"] {
            assert!(redirect_allowed(&url(&format!("https://{host}/a")), &url(&format!("https://{host}/b"))));
            assert!(!redirect_allowed(&url(&format!("https://{host}/a")), &url(&format!("http://{host}/b"))));
            assert!(!redirect_allowed(&url(&format!("https://{host}/a")), &url(&format!("https://{host}:8443/b"))));
        }
        let factory = url("https://factory.pixiv.net/a");
        assert!(!redirect_allowed(&factory, &url("https://booth.pximg.net/b")));
        assert!(!redirect_allowed(&factory, &url("https://evil.example/b")));
        assert!(!redirect_allowed(&factory, &url("https://127.0.0.1/b")));
    }

    #[test]
    fn the_tenth_redirect_is_followed_and_the_eleventh_is_not() {
        let hop = |n: usize| Url::parse(&format!("https://booth.pximg.net/{n}")).unwrap();
        let requested = |redirects: usize| (0..=redirects).map(hop).collect::<Vec<_>>();
        // the 10th redirect: the original request and 9 followed redirects came before it
        assert!(follow_redirect(&requested(9), &hop(10)));
        assert!(!follow_redirect(&requested(10), &hop(11)));
        assert!(!follow_redirect(&requested(0), &Url::parse("https://evil.example/").unwrap()));
    }

    async fn serve_on_loopback(app: axum::Router) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        port
    }

    /// Loopback only: proves the client refuses an off-host redirect that reqwest's default policy follows. Both URLs
    /// are IP literals, so the refusal is the redirect policy's and not `PublicOnly`'s.
    #[tokio::test]
    async fn the_client_refuses_an_off_host_redirect() {
        use axum::http::header::LOCATION;
        use axum::routing::get;
        let other = serve_on_loopback(axum::Router::new().route("/i", get(|| async { "image" }))).await;
        let app = axum::Router::new().route("/r", get(move || async move { (StatusCode::FOUND, [(LOCATION, format!("http://127.0.0.1:{other}/i"))]) }));
        let start = format!("http://127.0.0.1:{}/r", serve_on_loopback(app).await);
        assert!(fetch_upstream(&upstream_client().unwrap(), &start).await.is_err());
        // the same request with reqwest's default policy, so the refusal above is the policy's doing
        let default = reqwest::Client::builder().build().unwrap();
        assert_eq!(fetch_upstream(&default, &start).await.unwrap().as_deref(), Some(&b"image"[..]));
    }

    /// Loopback only: proves the client refuses a host name that resolves to loopback, which reqwest's default
    /// resolver connects to.
    #[tokio::test]
    async fn the_client_refuses_a_name_that_resolves_to_loopback() {
        let port = serve_on_loopback(axum::Router::new().route("/i", axum::routing::get(|| async { "image" }))).await;
        let url = format!("http://localhost:{port}/i");
        let refused = fetch_upstream(&upstream_client().unwrap(), &url).await.unwrap_err();
        assert!(format!("{refused:#}").contains("resolves to no public address"), "{refused:#}");
        let default = reqwest::Client::builder().build().unwrap();
        assert_eq!(fetch_upstream(&default, &url).await.unwrap().as_deref(), Some(&b"image"[..]));
    }
}
