//! Saved-search feeds: `/api/rss?<any search query string>` returns the newest matching items as RSS 2.0.
//! Stateless on purpose: the subscription *is* the URL, so it works without an account.

use std::sync::Arc;

use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};

use crate::params::Params;
use crate::search_request::{SearchRequest, Sort};
use crate::{ApiError, AppState, db, limit};

pub(crate) async fn rss_feed(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    s.limits.rss.check(ip)?;
    let _permit = s.acquire_search().await?;
    let user_q = q.unwrap_or_default();
    // the feed pins ordering and size, whatever the saved search sorts by
    let mut req = SearchRequest::from(&Params::parse(&user_q));
    (req.sort, req.asc, req.page, req.page_size) = (Sort::Newest, false, 1, 50);
    let now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&req, now, None, None, false))?;
    let items = db::load_items(&s.pool, &r.ids, false).await?;
    let link = format!("{}/?{user_q}", s.site_url);
    Ok(([(axum::http::header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")], render(&s.site_url, &link, &user_q, &items)).into_response())
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // XML 1.0 forbids most control characters; Booth text occasionally contains them
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// Path of the item's first image on the site's own origin, or `None` for no image. Mirrors `imgUrl` in
/// `web/src/lib/catalog.ts` at its default size, the image the result tile shows: change one, change the other.
fn image_path(it: &crate::db::ItemResponse) -> Option<String> {
    // an empty first entry counts as none, as in `imgUrl`
    if let Some(b) = it.booth_images.as_ref().and_then(|v| v.first()).filter(|b| !b.is_empty()) {
        if let Some(rest) = b.strip_prefix("https://booth.pximg.net/") {
            return Some(format!("/img/b/300x300_a2_g5/{rest}?format=avif"));
        }
        let rest = b.strip_prefix("https://factory.pixiv.net/files/uploads/i/conceptual_drawing/")?;
        let (uuid, file) = rest.split_once('/')?;
        let uuid_ok = uuid.len() == 36 && uuid.split('-').map(str::len).eq([8, 4, 4, 4, 12]) && uuid.split('-').all(hex_lower);
        let stem = file.strip_suffix(".png")?;
        let file_ok = (1..=120).contains(&stem.len()) && stem.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
        return (uuid_ok && file_ok).then(|| format!("/img/f/300x300_a2_g5/{uuid}/{file}?format=avif"));
    }
    let sha = it.media.first()?.rsplit_once("/booth/")?.1.strip_suffix(".webp")?;
    (sha.len() == 40 && hex_lower(sha)).then(|| format!("/img/v/{sha}.webp"))
}

fn hex_lower(s: &str) -> bool {
    s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

pub fn render(site_url: &str, channel_link: &str, query: &str, items: &[crate::db::ItemResponse]) -> String {
    let title = if query.is_empty() { "New items".to_owned() } else { format!("New items: {query}") };
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\"><channel><title>{}</title><link>{}</link><description>Newest Booth items matching this search</description>",
        esc(&title),
        esc(channel_link)
    );
    for it in items {
        let id = it.id;
        let link = format!("https://booth.pm/ja/items/{id}");
        let price = it.smart_price.or(it.price_jpy).map_or(String::new(), |p| format!("¥{p}"));
        let image = image_path(it).map_or(String::new(), |p| format!("<img src=\"{}\"/><br/>", esc(&format!("{site_url}{p}"))));
        let desc = format!("{image}{} · {}", esc(&it.author_name), esc(&price));
        let pub_date = it.published_at.map_or(String::new(), |d| format!("<pubDate>{}</pubDate>", d.to_rfc2822()));
        xml.push_str(&format!(
            "<item><title>{}</title><link>{link}</link><guid isPermaLink=\"true\">{link}</guid>{pub_date}<description>{}</description></item>",
            esc(&it.title),
            esc(&desc)
        ));
    }
    xml.push_str("</channel></rss>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://example.com";
    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
    const PX: &str = "https://booth.pximg.net/dc33a5b5-f8f7-418f-8087-393ea802b496/i/7/a.jpg";
    const FACTORY: &str = "https://factory.pixiv.net/files/uploads/i/conceptual_drawing/04cee5a2-f70c-49f2-b9b6-4c09a6b9bb74/canvas_94032c5836.png";

    fn item(booth_images: Option<Vec<&str>>, media: Vec<&str>) -> crate::db::ItemResponse {
        crate::db::ItemResponse {
            id: 7,
            booth_images: booth_images.map(|v| v.into_iter().map(str::to_owned).collect()),
            media: media.into_iter().map(str::to_owned).collect(),
            ..Default::default()
        }
    }

    fn feed(it: crate::db::ItemResponse) -> String {
        render(SITE, "https://example.com/?", "", &[it])
    }

    #[test]
    fn renders_escaped_items() {
        let item = crate::db::ItemResponse {
            id: 7,
            title: "Dress <&> \u{1}".to_owned(),
            author_name: "A&B".to_owned(),
            price_jpy: Some(800),
            smart_price: None,
            media: vec![format!("https://cdn.vrcatalogue.com/booth/{SHA}.webp")],
            published_at: Some(chrono::DateTime::parse_from_rfc3339("2026-09-24T08:00:01+00:00").unwrap().with_timezone(&chrono::Utc)),
            ..Default::default()
        };
        let xml = render(
            "https://example.com",
            "https://example.com/?q=a&b",
            "avatars=manuka",
            &[item],
        );
        assert!(xml.contains("<title>Dress &lt;&amp;&gt; </title>"));
        assert!(xml.contains("<link>https://example.com/?q=a&amp;b</link>"));
        assert!(xml.contains("<guid isPermaLink=\"true\">https://booth.pm/ja/items/7</guid>"));
        assert!(xml.contains("<pubDate>Thu, 24 Sep 2026 08:00:01 +0000</pubDate>"));
        // description is HTML carried inside XML: text is HTML-escaped, then the whole HTML is XML-escaped
        assert!(xml.contains("A&amp;amp;B · ¥800"));
        assert!(xml.contains(&format!("&lt;img src=&quot;https://example.com/img/v/{SHA}.webp&quot;/&gt;")));
    }

    #[test]
    fn booth_image_uses_the_tile_path_not_the_cdn() {
        let xml = feed(item(Some(vec![PX]), vec![&format!("https://cdn.vrcatalogue.com/booth/{SHA}.webp")]));
        assert!(xml.contains(
            "&lt;img src=&quot;https://example.com/img/b/300x300_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/7/a.jpg?format=avif&quot;/&gt;"
        ));
        assert!(!xml.contains("vrcatalogue"));
    }

    #[test]
    fn archive_media_maps_to_the_v_route() {
        let xml = feed(item(None, vec![&format!("https://cdn.vrcatalogue.com/booth/{SHA}.webp")]));
        assert!(xml.contains(&format!("https://example.com/img/v/{SHA}.webp")));
        assert!(!xml.contains("vrcatalogue"));
    }

    #[test]
    fn an_empty_gallery_falls_back_to_archive_media() {
        let xml = feed(item(Some(vec![]), vec![&format!("https://cdn.vrcatalogue.com/booth/{SHA}.webp")]));
        assert!(xml.contains(&format!("https://example.com/img/v/{SHA}.webp")));
    }

    #[test]
    fn factory_drawing_maps_to_the_f_route() {
        let xml = feed(item(Some(vec![FACTORY]), vec![]));
        assert!(xml.contains("https://example.com/img/f/300x300_a2_g5/04cee5a2-f70c-49f2-b9b6-4c09a6b9bb74/canvas_94032c5836.png?format=avif"));
    }

    /// The cases in `tests/fixtures/feed-image-paths.json` are shared with `imgUrl` in `web/src/lib/catalog.ts`:
    /// `web/src/lib/catalog.test.ts` reads the same file, so the two mappings must agree on every case listed there; an input the fixture lacks is not compared.
    #[test]
    fn image_path_matches_the_shared_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/feed-image-paths.json")).unwrap();
        let strings = |v: &serde_json::Value| v.as_array().map(|a| a.iter().map(|s| s.as_str().unwrap().to_owned()).collect::<Vec<_>>());
        let cases = fixture["cases"].as_array().unwrap();
        assert!(cases.len() > 20);
        for c in cases {
            let it = crate::db::ItemResponse {
                booth_images: strings(&c["booth_images"]),
                media: strings(&c["media"]).unwrap(),
                ..Default::default()
            };
            assert_eq!(image_path(&it).as_deref(), c["path"].as_str(), "{}", c["name"]);
        }
    }

    #[test]
    fn unservable_images_render_no_img() {
        let archive = format!("https://cdn.vrcatalogue.com/booth/{SHA}.webp");
        let upper = FACTORY.replace(".png", ".PNG");
        let jpg = FACTORY.replace(".png", ".jpg");
        let cases = [
            // the first booth image decides, even when archive media could serve
            item(Some(vec!["https://example.test/a.png"]), vec![]),
            item(Some(vec!["https://example.test/a.png"]), vec![&archive]),
            item(Some(vec![&upper]), vec![]),
            item(Some(vec![&jpg]), vec![]),
            item(Some(vec![&FACTORY.replace("04cee5a2", "04CEE5A2")]), vec![]),
            item(Some(vec![&FACTORY.replace(".png", "")]), vec![]),
            item(None, vec!["https://cdn.vrcatalogue.com/x.webp"]),
            item(None, vec![&format!("https://cdn.vrcatalogue.com/booth/{}.webp", SHA.to_uppercase())]),
            item(None, vec![]),
        ];
        for it in cases {
            assert!(!feed(it.clone()).contains("img"), "{it:?}");
        }
    }
}
