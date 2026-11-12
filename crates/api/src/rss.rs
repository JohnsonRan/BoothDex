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
    Ok(([(axum::http::header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")], render(&link, &user_q, &items)).into_response())
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

pub fn render(channel_link: &str, query: &str, items: &[crate::db::ItemResponse]) -> String {
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
        let image = it.media.first().map_or(String::new(), |u| format!("<img src=\"{}\"/><br/>", esc(u)));
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

    #[test]
    fn renders_escaped_items() {
        let item = crate::db::ItemResponse {
            id: 7,
            title: "Dress <&> \u{1}".to_owned(),
            author_name: "A&B".to_owned(),
            price_jpy: Some(800),
            smart_price: None,
            media: vec!["https://img/x.webp".to_owned()],
            published_at: Some(chrono::DateTime::parse_from_rfc3339("2026-09-24T08:00:01+00:00").unwrap().with_timezone(&chrono::Utc)),
            ..Default::default()
        };
        let xml = render(
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
        assert!(xml.contains("&lt;img src=&quot;https://img/x.webp&quot;/&gt;"));
    }
}
