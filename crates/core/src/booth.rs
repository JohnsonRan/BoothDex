//! Booth item JSON -> our `items` / `booth_images` rows. Shared by the worker (crawls) and the API
//! (refresh-on-open), so both write exactly the same columns.

use serde_json::{Value, json};
use sqlx::PgPool;

/// "¥ 1,200" / "¥ 300~" -> 1200 / 300
pub fn parse_price(s: &str) -> Option<i32> {
    let digits: String = s
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .filter(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

pub struct ItemFields {
    pub id: i64,
    pub row: Value,
    pub images: Vec<String>,
}

/// Columns our crawler owns. AI-derived columns (categories, avatars, translations…) are left to enrichment.
pub fn item_fields(v: &Value) -> Option<ItemFields> {
    let id = v["id"].as_i64()?;
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
    Some(ItemFields { id, row, images })
}

/// Title or description changed beyond whitespace. The imported archive kept descriptions with some whitespace
/// dropped, so on their first recrawl about half the items differed only there and were re-enriched for nothing.
macro_rules! text_changed {
    () => {
        "(regexp_replace(items.title, '\\s', '', 'g') is distinct from regexp_replace(excluded.title, '\\s', '', 'g')
          or regexp_replace(items.bio, '\\s', '', 'g') is distinct from regexp_replace(excluded.bio, '\\s', '', 'g'))"
    };
}

/// Upserts crawler-owned columns and the full gallery. New items are flagged for AI enrichment, and so are
/// existing ones whose title or description changed (their translations are stale). Returns true if inserted.
pub async fn store_item(pool: &PgPool, v: &Value) -> Result<bool, sqlx::Error> {
    let Some(f) = item_fields(v) else { return Err(sqlx::Error::Protocol("Booth item JSON without id".into())) };
    let inserted: bool = sqlx::query_scalar(concat!(
        "insert into items (id, title, bio, author_name, author_uri, author_icon, booth_category, price_jpy, likes, is_adult,
                            published_at, booth_tags, variations, status, scraped_at, needs_enrichment)
         select id, title, bio, author_name, author_uri, author_icon, booth_category, price_jpy, likes, is_adult,
                published_at, booth_tags, variations, 'ok', now(), true
         from jsonb_to_record($1) as x(id bigint, title text, bio text, author_name text, author_uri text, author_icon text,
                                       booth_category text, price_jpy integer, likes integer, is_adult boolean,
                                       published_at timestamptz, booth_tags text[], variations jsonb)
         on conflict (id) do update set
           needs_enrichment = items.needs_enrichment or ",
        text_changed!(),
        ",
           enrich_failures = case when ",
        text_changed!(),
        " then 0 else items.enrich_failures end,
           embedding = case when ",
        text_changed!(),
        " then null else items.embedding end,
           title = excluded.title, bio = excluded.bio, author_name = excluded.author_name, author_uri = excluded.author_uri,
           author_icon = excluded.author_icon, booth_category = excluded.booth_category, price_jpy = excluded.price_jpy,
           likes = excluded.likes, is_adult = excluded.is_adult, published_at = coalesce(excluded.published_at, items.published_at),
           booth_tags = excluded.booth_tags, variations = excluded.variations, status = 'ok', scraped_at = now()
         returning (xmax = 0)",
    ))
    .bind(&f.row)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "insert into booth_images (item_id, images, complete) values ($1, $2, true)
         on conflict (item_id) do update set images = excluded.images, complete = true, fetched_at = now()",
    )
    .bind(f.id)
    .bind(&f.images)
    .execute(pool)
    .await?;
    Ok(inserted)
}

pub async fn mark_delisted(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("update items set status = 'delisted', scraped_at = now() where id = $1").bind(id).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices() {
        assert_eq!(parse_price("¥ 1,200"), Some(1200));
        assert_eq!(parse_price("¥ 300~"), Some(300));
        assert_eq!(parse_price("¥ 0"), Some(0));
        assert_eq!(parse_price("free"), None);
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
        assert!(item_fields(&json!({})).is_none());
    }
}
