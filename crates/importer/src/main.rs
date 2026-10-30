//! Seeds Postgres from the VRCatalogue archive (vrcat-research/data).
//! Usage: DATABASE_URL=postgres://... cargo run -p importer -- <archive data dir>
//! Idempotent: rows are upserted, so it can be rerun over the same archive.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};

/// Archive ids are a mix of JSON numbers and numeric strings.
fn id_from_any<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Id {
        N(i64),
        S(String),
    }
    match Id::deserialize(d)? {
        Id::N(n) => Ok(n),
        Id::S(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

#[derive(Deserialize)]
struct ArcItem {
    #[serde(deserialize_with = "id_from_any")]
    id: i64,
    scraped_at: DateTime<Utc>,
    published_at: Option<String>,
    is_adult: bool,
    bio: Option<String>,
    item_status: String,
    title: String,
    author: String,
    author_uri: String,
    author_icon: Option<String>,
    category: Option<String>,
    price_jpy: Option<i32>,
    smart_price: Option<i32>,
    smart_price_reason: Option<String>,
    likes: Option<i32>,
    #[serde(default)]
    media: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    variations: Value,
    #[serde(default)]
    linked_avatars: Vec<ArcAvatar>,
    avatar_info: Option<ArcAvatar>,
    #[serde(default)]
    translations: Value,
    smart_categories: Option<Vec<ArcItemCategory>>,
    content_rating: Option<String>,
    quest_compatible: Option<String>,
}

#[derive(Deserialize, Clone)]
struct ArcAvatar {
    slug: String,
    en: String,
    ja: Option<String>,
    ko: Option<String>,
    image_url: Option<String>,
}

#[derive(Deserialize)]
struct ArcItemCategory {
    category: String,
    sub_category: Option<String>,
    is_primary: Option<bool>,
}

#[derive(Deserialize)]
struct ArcCategory {
    en: String,
    ja: Option<String>,
    ko: Option<String>,
    slug: String,
    mode: Option<String>,
    group_en: Option<String>,
    group_ja: Option<String>,
    group_ko: Option<String>,
    #[serde(default)]
    children: Vec<ArcCategory>,
}

#[derive(Deserialize)]
struct ArcTag {
    name: String,
    display: String,
    master_id: i32,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Deserialize)]
struct AnyId(#[serde(deserialize_with = "id_from_any")] i64);

#[derive(Deserialize)]
struct Label {
    ids: Vec<AnyId>,
}

/// One `items` row; field names must match the column list in `ITEM_COLS`.
#[derive(Serialize)]
struct ItemRow<'a> {
    id: i64,
    title: &'a str,
    bio: &'a str,
    author_name: &'a str,
    author_uri: &'a str,
    author_icon: Option<&'a str>,
    booth_category: Option<&'a str>,
    price_jpy: Option<i32>,
    smart_price: Option<i32>,
    smart_price_reason: Option<&'a str>,
    likes: i32,
    is_adult: bool,
    content_rating: Option<&'a str>,
    quest: Option<&'a str>,
    features: Vec<&'a str>,
    status: &'a str,
    published_at: Option<DateTime<Utc>>,
    scraped_at: DateTime<Utc>,
    original_language: Option<&'a str>,
    media: &'a [String],
    booth_tags: &'a [String],
    variations: &'a Value,
    translations: &'a Value,
    avatar_slug: Option<&'a str>,
}

const ITEM_COLS: &[(&str, &str)] = &[
    ("id", "bigint"),
    ("title", "text"),
    ("bio", "text"),
    ("author_name", "text"),
    ("author_uri", "text"),
    ("author_icon", "text"),
    ("booth_category", "text"),
    ("price_jpy", "integer"),
    ("smart_price", "integer"),
    ("smart_price_reason", "text"),
    ("likes", "integer"),
    ("is_adult", "boolean"),
    ("content_rating", "text"),
    ("quest", "text"),
    ("features", "text[]"),
    ("status", "text"),
    ("published_at", "timestamptz"),
    ("scraped_at", "timestamptz"),
    ("original_language", "text"),
    ("media", "text[]"),
    ("booth_tags", "text[]"),
    ("variations", "jsonb"),
    ("translations", "jsonb"),
    ("avatar_slug", "text"),
];

const FEATURES: &[&str] = &["ma", "liltoon", "poiyomi", "vrcfury"];

/// VRCatalogue emits either RFC 3339 or JS `Date.toString()` ("Thu Sep 24 2026 08:00:01 GMT+0000 (…)").
fn parse_published(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.to_utc());
    }
    let head = s.split(" (").next()?;
    DateTime::parse_from_str(head, "%a %b %d %Y %H:%M:%S GMT%z")
        .ok()
        .map(|d| d.to_utc())
}

fn read_gz<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let f = fs::File::open(path).with_context(|| path.display().to_string())?;
    serde_json::from_reader(GzDecoder::new(f)).with_context(|| path.display().to_string())
}

fn read_items(data: &Path, items: &mut HashMap<i64, ArcItem>) -> Result<()> {
    // *-delta = final snapshot of the newest pages, read last so it overrides older copies
    for set in ["items", "avatars", "items-delta", "avatars-delta"] {
        if !data.join(set).exists() {
            continue;
        }
        let mut pages: Vec<_> = fs::read_dir(data.join(set))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(|c: char| c.is_ascii_digit())))
            .collect();
        pages.sort();
        for page in pages {
            for mut it in read_gz::<Vec<ArcItem>>(&page)? {
                // both columns are NOT NULL jsonb
                if it.variations.is_null() {
                    it.variations = Value::Array(vec![]);
                }
                if it.translations.is_null() {
                    it.translations = Value::Object(Default::default());
                }
                items.insert(it.id, it);
            }
        }
    }
    Ok(())
}

async fn insert_categories(tx: &mut Transaction<'_, Postgres>, cats: &[ArcCategory]) -> Result<()> {
    for parent in cats {
        for (c, parent_slug) in std::iter::once((parent, None)).chain(parent.children.iter().map(|c| (c, Some(&parent.slug)))) {
            if !c.children.is_empty() && parent_slug.is_some() {
                bail!("unexpected third category level under {}", c.slug);
            }
            sqlx::query(
                "insert into smart_categories (slug, parent_slug, name_en, name_ja, name_ko, group_en, group_ja, group_ko, mode)
                 values ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 on conflict (slug) do update set parent_slug = excluded.parent_slug, name_en = excluded.name_en,
                   name_ja = excluded.name_ja, name_ko = excluded.name_ko, group_en = excluded.group_en,
                   group_ja = excluded.group_ja, group_ko = excluded.group_ko, mode = excluded.mode",
            )
            .bind(&c.slug)
            .bind(parent_slug)
            .bind(&c.en)
            .bind(&c.ja)
            .bind(&c.ko)
            .bind(&c.group_en)
            .bind(&c.group_ja)
            .bind(&c.group_ko)
            .bind(&c.mode)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let data = std::env::args().nth(1).context("usage: importer <archive data dir>")?;
    let data = Path::new(&data);
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").context("DATABASE_URL not set")?).await?;
    catalogue_core::MIGRATOR.run(&pool).await?;

    let mut items = HashMap::new();
    read_items(data, &mut items)?;
    println!("items: {}", items.len());

    let cats: Vec<ArcCategory> = serde_json::from_value(read_gz::<Value>(&data.join("dict/smart-categories.json.gz"))?["categories"].take())?;
    let tags: Vec<ArcTag> = read_gz(&data.join("dict/tags.json.gz"))?;
    let dict_avatars: Vec<ArcAvatar> = read_gz(&data.join("dict/avatars.json.gz"))?;

    let label = |name: &str| -> Result<HashSet<i64>> {
        let p = data.join(format!("labels/{name}.json"));
        Ok(serde_json::from_slice::<Label>(&fs::read(&p).with_context(|| p.display().to_string())?)?.ids.into_iter().map(|AnyId(id)| id).collect())
    };
    let feature_sets: Vec<(&str, HashSet<i64>)> = FEATURES.iter().map(|f| Ok((*f, label(&format!("feature-{f}"))?))).collect::<Result<_>>()?;
    let (female, male) = (label("gender-female")?, label("gender-male")?);

    // category lookup by English names, as items reference categories by name
    let mut cat_slug: HashMap<(&str, Option<&str>), &str> = HashMap::new();
    for p in &cats {
        cat_slug.insert((&p.en, None), &p.slug);
        for c in &p.children {
            cat_slug.insert((&p.en, Some(&c.en)), &c.slug);
        }
    }

    // avatars: dictionary first, then anything only seen on items; the avatar's own item = most-liked item carrying avatar_info
    let mut avatars: HashMap<String, (ArcAvatar, Option<(i64, i32)>, Option<&str>)> = HashMap::new();
    for a in dict_avatars {
        avatars.insert(a.slug.clone(), (a, None, None));
    }
    for it in items.values() {
        for a in &it.linked_avatars {
            avatars.entry(a.slug.clone()).or_insert_with(|| (a.clone(), None, None));
        }
        if let Some(a) = &it.avatar_info {
            let e = avatars.entry(a.slug.clone()).or_insert_with(|| (a.clone(), None, None));
            let likes = it.likes.unwrap_or(0);
            if e.1.is_none_or(|(_, l)| likes > l) {
                e.1 = Some((it.id, likes));
                e.2 = if female.contains(&it.id) { Some("female") } else if male.contains(&it.id) { Some("male") } else { None };
            }
        }
    }

    let mut tx = pool.begin().await?;
    insert_categories(&mut tx, &cats).await?;

    for t in &tags {
        sqlx::query(
            "insert into smart_tags (id, name, display, aliases) values ($1,$2,$3,$4)
             on conflict (id) do update set name = excluded.name, display = excluded.display, aliases = excluded.aliases",
        )
        .bind(t.master_id)
        .bind(&t.name)
        .bind(&t.display)
        .bind(&t.aliases)
        .execute(&mut *tx)
        .await?;
    }

    for (a, own, gender) in avatars.values() {
        sqlx::query(
            "insert into avatars (slug, name_en, name_ja, name_ko, image_url, avatar_item_id, gender) values ($1,$2,$3,$4,$5,$6,$7)
             on conflict (slug) do update set name_en = excluded.name_en, name_ja = excluded.name_ja, name_ko = excluded.name_ko,
               image_url = excluded.image_url, avatar_item_id = excluded.avatar_item_id, gender = excluded.gender",
        )
        .bind(&a.slug)
        .bind(&a.en)
        .bind(&a.ja)
        .bind(&a.ko)
        .bind(&a.image_url)
        .bind(own.map(|(id, _)| id))
        .bind(gender)
        .execute(&mut *tx)
        .await?;
    }

    let cols = ITEM_COLS.iter().map(|(c, _)| *c).collect::<Vec<_>>().join(", ");
    let typed = ITEM_COLS.iter().map(|(c, t)| format!("{c} {t}")).collect::<Vec<_>>().join(", ");
    let updates = ITEM_COLS[1..].iter().map(|(c, _)| format!("{c} = excluded.{c}")).collect::<Vec<_>>().join(", ");
    let insert_items = format!("insert into items ({cols}) select {cols} from jsonb_to_recordset($1) as x({typed}) on conflict (id) do update set {updates}");

    let mut sorted: Vec<&ArcItem> = items.values().collect();
    sorted.sort_by_key(|it| it.id);
    let (mut bad_dates, mut unmapped_cats) = (0usize, HashMap::<String, usize>::new());
    let (mut link_items, mut link_avatars) = (Vec::new(), Vec::new());
    let (mut cat_items, mut cat_slugs, mut cat_primary) = (Vec::new(), Vec::new(), Vec::new());

    for chunk in sorted.chunks(1000) {
        let rows: Vec<ItemRow> = chunk
            .iter()
            .map(|it| {
                let published_at = it.published_at.as_deref().and_then(parse_published);
                if it.published_at.is_some() && published_at.is_none() {
                    bad_dates += 1;
                }
                ItemRow {
                    id: it.id,
                    title: &it.title,
                    bio: it.bio.as_deref().unwrap_or(""),
                    author_name: &it.author,
                    author_uri: &it.author_uri,
                    author_icon: it.author_icon.as_deref(),
                    booth_category: it.category.as_deref(),
                    price_jpy: it.price_jpy,
                    smart_price: it.smart_price,
                    smart_price_reason: it.smart_price_reason.as_deref(),
                    likes: it.likes.unwrap_or(0),
                    is_adult: it.is_adult,
                    content_rating: it.content_rating.as_deref(),
                    quest: it.quest_compatible.as_deref(),
                    features: feature_sets.iter().filter(|(_, s)| s.contains(&it.id)).map(|(f, _)| *f).collect(),
                    status: &it.item_status,
                    published_at,
                    scraped_at: it.scraped_at,
                    original_language: it.translations.get("original_language").and_then(Value::as_str),
                    media: &it.media,
                    booth_tags: &it.tags,
                    variations: &it.variations,
                    translations: &it.translations,
                    avatar_slug: it.avatar_info.as_ref().map(|a| a.slug.as_str()),
                }
            })
            .collect();
        // built only from the ITEM_COLS constant, no user input
        sqlx::query(sqlx::AssertSqlSafe(insert_items.as_str())).bind(sqlx::types::Json(&rows)).execute(&mut *tx).await?;

        for it in chunk {
            for a in &it.linked_avatars {
                link_items.push(it.id);
                link_avatars.push(a.slug.as_str());
            }
            let mut seen = HashSet::new();
            for c in it.smart_categories.iter().flatten() {
                match cat_slug.get(&(c.category.as_str(), c.sub_category.as_deref())) {
                    Some(slug) if seen.insert(*slug) => {
                        cat_items.push(it.id);
                        cat_slugs.push(*slug);
                        cat_primary.push(c.is_primary.unwrap_or(false));
                    }
                    Some(_) => {}
                    None => *unmapped_cats.entry(format!("{} / {:?}", c.category, c.sub_category)).or_default() += 1,
                }
            }
        }
    }

    sqlx::query("insert into item_avatars (item_id, avatar_slug) select * from unnest($1::bigint[], $2::text[]) on conflict do nothing")
        .bind(&link_items)
        .bind(&link_avatars)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "insert into item_categories (item_id, category_slug, is_primary) select * from unnest($1::bigint[], $2::text[], $3::boolean[])
         on conflict (item_id, category_slug) do update set is_primary = excluded.is_primary",
    )
    .bind(&cat_items)
    .bind(&cat_slugs)
    .bind(&cat_primary)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    println!(
        "avatars: {}, item-avatar links: {}, item-category links: {}, unparsed dates: {bad_dates}",
        avatars.len(),
        link_items.len(),
        cat_items.len()
    );
    if !unmapped_cats.is_empty() {
        println!("unmapped categories ({} kinds): {:?}", unmapped_cats.len(), unmapped_cats.iter().take(20).collect::<Vec<_>>());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_published;

    #[test]
    fn parses_both_archive_date_formats() {
        let iso = parse_published("2018-06-27T02:34:14.000Z").unwrap();
        assert_eq!(iso.to_rfc3339(), "2018-06-27T02:34:14+00:00");
        let js = parse_published("Thu Sep 24 2026 08:00:01 GMT+0000 (Coordinated Universal Time)").unwrap();
        assert_eq!(js.to_rfc3339(), "2026-09-24T08:00:01+00:00");
        assert!(parse_published("garbage").is_none());
    }
}
