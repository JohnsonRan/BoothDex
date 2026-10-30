//! Booth crawler.
//!   crawler listing [--tag VRChat] [--from 1] [--delay-ms 1000]
//!       Walks Booth search pages (60 items each); stores up to 4 image URLs per item. ~3.4k pages for VRChat.
//!   crawler details [--delay-ms 500] [--limit N]
//!       Fetches items/<id>.json for items that have no image row yet; stores the full gallery.
//!       A 404 marks the item delisted (verified 2026-10-31: Booth's own page 404s too, even in a browser);
//!       `listing` flips it back to ok if the item shows up on a public listing again.
//! Needs DATABASE_URL. Rerun-safe: listing never overwrites a full gallery with a partial one.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use regex::Regex;
use sqlx::PgPool;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36";

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// GET with retry/backoff on 429, 5xx and network errors. `Ok(None)` = 404.
async fn get(client: &reqwest::Client, url: &str) -> Result<Option<String>> {
    for attempt in 0..6u32 {
        match client.get(url).send().await {
            Ok(r) if r.status() == 404 => return Ok(None),
            Ok(r) if r.status().is_success() => return Ok(Some(r.text().await?)),
            Ok(r) if r.status() == 429 || r.status().is_server_error() => {
                let wait = r.headers().get("retry-after").and_then(|v| v.to_str().ok()?.parse().ok()).unwrap_or(2u64.pow(attempt + 1));
                eprintln!("{} on {url}, waiting {wait}s", r.status());
                tokio::time::sleep(Duration::from_secs(wait)).await;
            }
            Ok(r) => bail!("{} on {url}", r.status()),
            Err(e) => {
                eprintln!("{e:?} on {url}");
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt + 1))).await;
            }
        }
    }
    bail!("giving up on {url}")
}

/// Item id -> canonical image URLs (size prefix stripped), in page order.
fn parse_listing(html: &str, img: &Regex) -> Vec<(i64, Vec<String>)> {
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

fn image_regex() -> Regex {
    Regex::new(r#"https://booth\.pximg\.net/(?:c/[^/"'\s]+/)?([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})/i/(\d+)/([^"'\s)?]+)"#).unwrap()
}

async fn listing(pool: &PgPool, client: &reqwest::Client) -> Result<()> {
    let tag = arg("--tag").unwrap_or_else(|| "VRChat".into());
    let delay = Duration::from_millis(arg("--delay-ms").and_then(|v| v.parse().ok()).unwrap_or(1000));
    let from: u32 = arg("--from").and_then(|v| v.parse().ok()).unwrap_or(1);
    let img = image_regex();
    let pager = Regex::new(r"[?&]page=(\d+)").unwrap();
    let url = |p: u32| format!("https://booth.pm/ja/items?tags%5B%5D={}&sort=new&page={p}", urlencode(&tag));

    let first = get(client, &url(1)).await?.context("listing page 1 missing")?;
    let last = pager.captures_iter(&first).filter_map(|c| c[1].parse::<u32>().ok()).max().unwrap_or(1);
    println!("tag {tag}: pages {from}..={last}");
    let (mut items, mut images) = (0usize, 0usize);
    for p in from..=last {
        let html = if p == 1 { first.clone() } else { get(client, &url(p)).await?.unwrap_or_default() };
        let rows = parse_listing(&html, &img);
        if rows.is_empty() {
            eprintln!("page {p}: no items");
        }
        items += rows.len();
        images += rows.iter().map(|r| r.1.len()).sum::<usize>();
        let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
        let imgs: Vec<String> = rows.iter().map(|r| serde_json::to_string(&r.1).unwrap()).collect();
        sqlx::query(
            "insert into booth_images (item_id, images, complete)
             select id, array(select jsonb_array_elements_text(imgs::jsonb)), false from unnest($1::bigint[], $2::text[]) as t(id, imgs)
             on conflict (item_id) do update set images = excluded.images, fetched_at = now() where not booth_images.complete",
        )
        .bind(&ids)
        .bind(&imgs)
        .execute(pool)
        .await?;
        // an item seen on a public listing is live again (scheduled/private items 404 for a while)
        sqlx::query("update items set status = 'ok' where id = any($1) and status = 'delisted'").bind(&ids).execute(pool).await?;
        if p % 50 == 0 || p == last {
            println!("page {p}/{last}: {items} items, {images} images");
        }
        tokio::time::sleep(delay).await;
    }
    Ok(())
}

async fn details(pool: &PgPool, client: &reqwest::Client) -> Result<()> {
    let delay = Duration::from_millis(arg("--delay-ms").and_then(|v| v.parse().ok()).unwrap_or(500));
    let limit: i64 = arg("--limit").and_then(|v| v.parse().ok()).unwrap_or(i64::MAX);
    let ids: Vec<i64> = sqlx::query_scalar(
        "select i.id from items i where i.status = 'ok' and not exists (select 1 from booth_images b where b.item_id = i.id) order by i.id desc limit $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    println!("{} items without images", ids.len());
    let (mut ok, mut gone) = (0usize, 0usize);
    for (n, id) in ids.iter().enumerate() {
        match get(client, &format!("https://booth.pm/ja/items/{id}.json")).await? {
            Some(body) => {
                let v: serde_json::Value = serde_json::from_str(&body).with_context(|| format!("item {id}"))?;
                let images: Vec<String> = v["images"].as_array().into_iter().flatten().filter_map(|i| i["original"].as_str().map(str::to_owned)).collect();
                sqlx::query(
                    "insert into booth_images (item_id, images, complete) values ($1, $2, true)
                     on conflict (item_id) do update set images = excluded.images, complete = true, fetched_at = now()",
                )
                .bind(id)
                .bind(&images)
                .execute(pool)
                .await?;
                ok += 1;
            }
            None => {
                sqlx::query("update items set status = 'delisted' where id = $1").bind(id).execute(pool).await?;
                gone += 1;
            }
        }
        if (n + 1) % 200 == 0 || n + 1 == ids.len() {
            println!("{}/{}: {ok} stored, {gone} delisted", n + 1, ids.len());
        }
        tokio::time::sleep(delay).await;
    }
    Ok(())
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider().install_default().ok();
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").context("DATABASE_URL not set")?).await?;
    catalogue_core::MIGRATOR.run(&pool).await?;
    let client = reqwest::Client::builder().user_agent(UA).timeout(Duration::from_secs(30)).build()?;
    match std::env::args().nth(1).as_deref() {
        Some("listing") => listing(&pool, &client).await,
        Some("details") => details(&pool, &client).await,
        _ => bail!("usage: crawler listing|details [options]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_images_by_item_and_strips_size() {
        let html = r#"<img data-original="https://booth.pximg.net/c/300x300_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/aaa_base_resized.jpg">
            <img data-original="https://booth.pximg.net/c/300x300_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/bbb_base_resized.jpg">
            <img src="https://booth.pximg.net/c/72x72_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/aaa_base_resized.jpg">
            <img data-original="https://booth.pximg.net/c/300x300_a2_g5/20f6879b-5d59-4056-9dc4-b98220aaf8c5/i/8823199/ccc_base_resized.png">"#;
        let rows = parse_listing(html, &image_regex());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, 9064622);
        assert_eq!(
            rows[0].1,
            [
                "https://booth.pximg.net/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/aaa_base_resized.jpg",
                "https://booth.pximg.net/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/bbb_base_resized.jpg"
            ]
        );
        assert_eq!(rows[1].1, ["https://booth.pximg.net/20f6879b-5d59-4056-9dc4-b98220aaf8c5/i/8823199/ccc_base_resized.png"]);
    }

    #[test]
    fn encodes_tag() {
        assert_eq!(urlencode("VRChat"), "VRChat");
        assert_eq!(urlencode("衣装 x"), "%E8%A1%A3%E8%A3%85%20x");
    }
}
