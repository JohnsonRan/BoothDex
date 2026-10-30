//! Booth HTTP access: polite client with retry/backoff, request counters, listing and item parsing.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36";

#[derive(Default)]
pub struct Counters {
    pub requests: AtomicU64,
    pub errors: AtomicU64,
    pub rate_limited: AtomicU64,
}

pub struct Booth {
    client: reqwest::Client,
    pub counters: Counters,
    img: Regex,
    pager: Regex,
}

impl Booth {
    pub fn new() -> Result<Self> {
        rustls::crypto::ring::default_provider().install_default().ok();
        Ok(Self {
            client: reqwest::Client::builder().user_agent(UA).timeout(Duration::from_secs(30)).build()?,
            counters: Counters::default(),
            img: Regex::new(r#"https://booth\.pximg\.net/(?:c/[^/"'\s]+/)?([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})/i/(\d+)/([^"'\s)?]+)"#)?,
            pager: Regex::new(r"[?&]page=(\d+)")?,
        })
    }

    /// GET with retry/backoff on 429, 5xx and network errors. `Ok(None)` = 404.
    pub async fn get(&self, url: &str) -> Result<Option<String>> {
        for attempt in 0..6u32 {
            self.counters.requests.fetch_add(1, Ordering::Relaxed);
            match self.client.get(url).send().await {
                Ok(r) if r.status() == 404 => return Ok(None),
                Ok(r) if r.status().is_success() => return Ok(Some(r.text().await?)),
                Ok(r) if r.status() == 429 || r.status().is_server_error() => {
                    if r.status() == 429 {
                        self.counters.rate_limited.fetch_add(1, Ordering::Relaxed);
                    } else {
                        self.counters.errors.fetch_add(1, Ordering::Relaxed);
                    }
                    let wait = r.headers().get("retry-after").and_then(|v| v.to_str().ok()?.parse().ok()).unwrap_or(2u64.pow(attempt + 1));
                    tokio::time::sleep(Duration::from_secs(wait.min(300))).await;
                }
                Ok(r) => {
                    self.counters.errors.fetch_add(1, Ordering::Relaxed);
                    bail!("{} on {url}", r.status());
                }
                Err(e) => {
                    self.counters.errors.fetch_add(1, Ordering::Relaxed);
                    if attempt == 5 {
                        bail!("{e:?} on {url}");
                    }
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt + 1))).await;
                }
            }
        }
        bail!("giving up on {url}")
    }

    /// Item JSON from the English site, so the category name matches the archive ("3D Clothing").
    pub async fn item(&self, id: i64) -> Result<Option<Value>> {
        match self.get(&format!("https://booth.pm/en/items/{id}.json")).await? {
            Some(body) => Ok(Some(serde_json::from_str(&body).with_context(|| format!("item {id}"))?)),
            None => Ok(None),
        }
    }

    /// One newest-first search page: (item id, up to 4 canonical image URLs), and the last page number.
    pub async fn listing(&self, tag: &str, page: u32) -> Result<(Vec<(i64, Vec<String>)>, u32)> {
        let url = format!("https://booth.pm/ja/items?tags%5B%5D={}&sort=new&page={page}", urlencode(tag));
        let html = self.get(&url).await?.unwrap_or_default();
        let last = self.pager.captures_iter(&html).filter_map(|c| c[1].parse::<u32>().ok()).max().unwrap_or(page);
        Ok((parse_listing(&html, &self.img), last))
    }
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

pub fn urlencode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// "¥ 1,200" / "¥ 300~" -> 1200 / 300
pub fn parse_price(s: &str) -> Option<i32> {
    let digits: String = s.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit() || *c == ',').filter(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Booth item JSON -> the columns our crawler owns. AI-derived columns are left to enrichment.
pub struct ItemFields {
    pub id: i64,
    pub row: Value,
    pub images: Vec<String>,
}

pub fn item_fields(v: &Value) -> Result<ItemFields> {
    let id = v["id"].as_i64().context("item json without id")?;
    let variations: Vec<Value> = v["variations"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|x| json!({ "id": x["id"].to_string(), "name": x["name"].as_str().unwrap_or_default(), "price_jpy": x["price"], "status": x["status"], "type": x["type"] }))
        .collect();
    let row = json!({
        "id": id,
        "title": v["name"].as_str().unwrap_or_default(),
        "bio": v["description"].as_str().unwrap_or_default(),
        "author_name": v["shop"]["name"].as_str().unwrap_or_default(),
        "author_uri": v["shop"]["url"].as_str().unwrap_or_default(),
        "author_icon": v["shop"]["thumbnail_url"],
        "booth_category": v["category"]["name"],
        "price_jpy": v["price"].as_str().and_then(parse_price),
        "likes": v["wish_lists_count"].as_i64().unwrap_or(0),
        "is_adult": v["is_adult"].as_bool().unwrap_or(false),
        "published_at": v["published_at"],
        "booth_tags": v["tags"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str()).collect::<Vec<_>>(),
        "variations": variations,
    });
    let images = v["images"].as_array().into_iter().flatten().filter_map(|i| i["original"].as_str().map(str::to_owned)).collect();
    Ok(ItemFields { id, row, images })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn prices_and_encoding() {
        assert_eq!(parse_price("¥ 1,200"), Some(1200));
        assert_eq!(parse_price("¥ 300~"), Some(300));
        assert_eq!(parse_price("¥ 0"), Some(0));
        assert_eq!(parse_price("free"), None);
        assert_eq!(urlencode("衣装 x"), "%E8%A1%A3%E8%A3%85%20x");
    }

    #[test]
    fn maps_item_json() {
        let v = json!({"id": 9071184, "name": "Outfit", "description": "d", "price": "¥ 399", "wish_lists_count": 4, "is_adult": false,
            "published_at": "2026-10-30T19:38:12.000+09:00", "category": {"name": "3D Clothing"},
            "shop": {"name": "stlw", "url": "https://stlw.booth.pm/", "thumbnail_url": "https://booth.pximg.net/x.png"},
            "tags": [{"name": "ショコラ"}], "images": [{"original": "https://booth.pximg.net/a/i/9071184/b.jpg"}],
            "variations": [{"id": 15147008, "name": null, "price": 399, "status": "addable_to_cart", "type": "digital"}]});
        let f = item_fields(&v).unwrap();
        assert_eq!(f.id, 9071184);
        assert_eq!(f.row["price_jpy"], 399);
        assert_eq!(f.row["booth_category"], "3D Clothing");
        assert_eq!(f.row["variations"][0]["id"], "15147008");
        assert_eq!(f.row["variations"][0]["name"], "");
        assert_eq!(f.images.len(), 1);
    }
}
