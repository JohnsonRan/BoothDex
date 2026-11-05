//! Booth listing pages: walks newest-first search results and parses item cards and image links.
//! HTTP (user agent, retry/backoff, counters) lives in `boothdex_core::booth_client`.

use std::time::Duration;

use anyhow::Result;
use regex::Regex;
use scraper::{Html, Selector};

use boothdex_core::booth_client::BoothClient;
use boothdex_core::urlencode;

pub struct Booth {
    pub http: BoothClient,
    img: Regex,
    pager: Regex,
}

impl Booth {
    pub fn new() -> Result<Self> {
        rustls::crypto::ring::default_provider().install_default().ok();
        Ok(Self {
            http: BoothClient::new(6, Duration::from_secs(30))?,
            img: Regex::new(r#"https://booth\.pximg\.net/(?:c/[^/"'\s]+/)?([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})/i/(\d+)/([^"'\s)?]+)"#)?,
            pager: Regex::new(r"[?&]page=(\d+)")?,
        })
    }

    /// One newest-first search page as raw HTML, plus the last page number.
    pub async fn listing_html(&self, tag: &str, page: u32) -> Result<(String, u32)> {
        let url = format!("https://booth.pm/ja/items?tags%5B%5D={}&sort=new&page={page}", urlencode(tag));
        let html = self.http.get(&url).await?.unwrap_or_default();
        let last = self.pager.captures_iter(&html).filter_map(|c| c[1].parse::<u32>().ok()).max().unwrap_or(page);
        Ok((html, last))
    }

    /// One newest-first search page: (item id, up to 4 canonical image URLs), and the last page number.
    pub async fn listing(&self, tag: &str, page: u32) -> Result<(Vec<(i64, Vec<String>)>, u32)> {
        let (html, last) = self.listing_html(tag, page).await?;
        Ok((parse_listing(&html, &self.img), last))
    }

    pub fn images(&self, html: &str) -> Vec<(i64, Vec<String>)> {
        parse_listing(html, &self.img)
    }
}

/// (item id, listed price) from the item cards on a listing page.
pub fn parse_cards(html: &str) -> Vec<(i64, Option<i32>)> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("li.item-card").expect("valid item-card selector");
    document.select(&selector)
        .filter_map(|card| {
            let card = card.value();
            Some((card.attr("data-product-id")?.parse().ok()?, card.attr("data-product-price").and_then(|p| p.parse().ok())))
        })
        .collect()
}


/// Item id -> canonical image URLs (size prefix stripped), in page order.
pub fn parse_listing(html: &str, img: &Regex) -> Vec<(i64, Vec<String>)> {
    let mut out: Vec<(i64, Vec<String>)> = Vec::new();
    for c in img.captures_iter(html) {
        let Ok(id) = c[2].parse::<i64>() else { continue };
        let url = format!("https://booth.pximg.net/{}/i/{}/{}", &c[1], &c[2], &c[3]);
        let pos = out.iter().position(|(i, _)| *i == id).unwrap_or_else(|| {
            out.push((id, Vec::new()));
            out.len() - 1
        });
        if !out[pos].1.contains(&url) {
            out[pos].1.push(url);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[tokio::test(start_paused = true)]
    async fn exhausted_retries_do_not_sleep_and_keep_retry_after() {
        for (status, retry_after, seconds) in [(429, Some("3"), 15), (503, None, 62)] {
            let (url, requests) = crate::retry_test_server(status.try_into().unwrap(), retry_after).await;
            let booth = BoothClient::from_reqwest(reqwest::Client::builder().no_proxy().pool_max_idle_per_host(0).build().unwrap(), 6);
            let start = tokio::time::Instant::now();
            assert!(booth.get(&url).await.unwrap_err().to_string().contains("giving up"));
            assert_eq!(requests.load(Ordering::Relaxed), 6);
            assert_eq!(booth.counters.requests.load(Ordering::Relaxed), 6);
            assert_eq!(booth.counters.rate_limited.load(Ordering::Relaxed), if status == 429 { 6 } else { 0 });
            assert_eq!(booth.counters.errors.load(Ordering::Relaxed), if status == 503 { 6 } else { 0 });
            assert_eq!(start.elapsed(), Duration::from_secs(seconds));
        }
    }

    #[test]
    fn groups_images_by_item_and_strips_size() {
        let b = Booth::new().unwrap();
        let html = r#"<img data-original="https://booth.pximg.net/c/300x300_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/aaa_base_resized.jpg">
            <img data-original="https://booth.pximg.net/c/300x300_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/bbb_base_resized.jpg">
            <img src="https://booth.pximg.net/c/72x72_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/aaa_base_resized.jpg">
            <img data-original="https://booth.pximg.net/c/300x300_a2_g5/20f6879b-5d59-4056-9dc4-b98220aaf8c5/i/8823199/ccc_base_resized.png">"#;
        let rows = parse_listing(html, &b.img);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].1.len(), 2);
        assert_eq!(rows[1].1, ["https://booth.pximg.net/20f6879b-5d59-4056-9dc4-b98220aaf8c5/i/8823199/ccc_base_resized.png"]);
    }

    #[test]
    fn parses_cards() {
        let html = r#"<ul><li class="item-card l-card " data-product-brand="kitunesann" data-product-id="9070052" data-product-name="x" data-product-price="1300" data-tracking="impression_item"><div>
            <li class="item-card l-card " data-product-id="9070053" data-product-price=""><div></ul>"#;
        assert_eq!(parse_cards(html), [(9070052, Some(1300)), (9070053, None)]);
    }

    #[test]
    fn parses_cards_with_html_attribute_variants() {
        let html = r#"<ul>
            <li data-product-name='x > y' data-product-price='1300' data-product-id='9070052' class='l-card item-card featured'>
            <li data-product-id=9070053 class=item-card data-product-price=0>
            <li class="item-card" data-product-price="&#49;200" data-product-id="9070054">
            </ul>"#;
        assert_eq!(parse_cards(html), [(9070052, Some(1300)), (9070053, Some(0)), (9070054, Some(1200))]);
    }

    #[test]
    fn parses_cards_with_missing_or_invalid_attributes() {
        let html = r#"<ul>
            <li class="item-card" data-product-id="1">
            <li class="item-card" data-product-id="2" data-product-price="">
            <li class="item-card" data-product-id="3" data-product-price="invalid">
            <li class="item-card" data-product-id="invalid" data-product-price="100">
            <li class="item-card" data-product-price="100">
            <li class="item-card-other" data-product-id="4" data-product-price="100">
            </ul>
            <div class="item-card" data-product-id="5" data-product-price="100"></div>"#;
        assert_eq!(parse_cards(html), [(1, None), (2, None), (3, None)]);
        assert!(parse_cards("").is_empty());
    }
}
