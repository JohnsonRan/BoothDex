use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::catalog::DocInput;

/// Explicit typed representation of an item in API responses.
///
/// Internal columns like `embedding` (vectors) are deliberately omitted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ItemResponse {
    pub id: i64,
    pub title: String,
    pub bio: String,
    pub author_name: String,
    pub author_uri: String,
    pub author_icon: Option<String>,
    pub booth_category: Option<String>,
    pub price_jpy: Option<i32>,
    pub smart_price: Option<i32>,
    pub smart_price_reason: Option<String>,
    pub likes: i32,
    pub is_adult: bool,
    pub content_rating: Option<String>,
    pub quest: Option<String>,
    pub features: Vec<String>,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub scraped_at: DateTime<Utc>,
    pub original_language: Option<String>,
    pub avatar_slug: Option<String>,
    pub supported: bool,
    pub gender: Option<String>,
    pub needs_enrichment: bool,
    pub last_listed_at: Option<DateTime<Utc>>,
    pub enrich_failures: i16,
    pub media: Vec<String>,
    pub booth_tags: Vec<String>,
    pub variations: Value,
    pub translations: Value,
    pub linked_avatars: Value,
    pub categories: Value,
    pub booth_images: Option<Vec<String>>,
    pub booth_images_complete: Option<bool>,
}

impl Default for ItemResponse {
    fn default() -> Self {
        Self {
            id: 0,
            title: String::new(),
            bio: String::new(),
            author_name: String::new(),
            author_uri: String::new(),
            author_icon: None,
            booth_category: None,
            price_jpy: None,
            smart_price: None,
            smart_price_reason: None,
            likes: 0,
            is_adult: false,
            content_rating: None,
            quest: None,
            features: Vec::new(),
            status: "ok".to_owned(),
            published_at: None,
            scraped_at: DateTime::UNIX_EPOCH,
            original_language: None,
            avatar_slug: None,
            supported: false,
            gender: None,
            needs_enrichment: false,
            last_listed_at: None,
            enrich_failures: 0,
            media: Vec::new(),
            booth_tags: Vec::new(),
            variations: serde_json::json!([]),
            translations: serde_json::json!({}),
            linked_avatars: serde_json::json!([]),
            categories: serde_json::json!([]),
            booth_images: None,
            booth_images_complete: None,
        }
    }
}

pub(crate) fn order_items(ids: &[i64], mut by_id: HashMap<i64, ItemResponse>) -> Vec<ItemResponse> {
    ids.iter().filter_map(|id| by_id.remove(id)).collect()
}

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    title: String,
    titles_translated: Vec<String>,
    bio: String,
    author_name: String,
    booth_category: Option<String>,
    price: Option<i32>,
    likes: i32,
    published: Option<i64>,
    is_adult: bool,
    content_rating: Option<String>,
    quest: Option<String>,
    features: Vec<String>,
    booth_tags: Vec<String>,
    supported: bool,
    gender: Option<String>,
    avatars: Vec<String>,
    cats: Vec<String>,
    cats_primary: Vec<String>,
}

pub async fn load_docs(pool: &PgPool) -> Result<Vec<DocInput>> {
    // smart tags: a booth tag maps to a master tag when it equals (case-insensitively) its name, display or an alias
    let mut alias: HashMap<String, String> = HashMap::new();
    for (name, display, aliases) in sqlx::query_as::<_, (String, String, Vec<String>)>("select name, display, aliases from smart_tags").fetch_all(pool).await? {
        for a in aliases.iter().chain([&name, &display]) {
            alias.insert(a.to_lowercase(), name.clone());
        }
    }
    let parent: HashMap<String, String> =
        sqlx::query_as::<_, (String, String)>("select slug, parent_slug from smart_categories where parent_slug is not null").fetch_all(pool).await?.into_iter().collect();

    // only titles are translated (titles_translated); descriptions are searched in their original language
    let rows: Vec<Row> = sqlx::query_as(
        "select i.id, i.title, i.bio, i.author_name, i.booth_category, coalesce(i.smart_price, i.price_jpy) as price, i.likes,
                extract(epoch from i.published_at)::bigint as published, i.is_adult, i.content_rating, i.quest, i.features,
                i.booth_tags, i.supported, i.gender,
                array_remove(array[i.translations->'en'->>'title', i.translations->'ja'->>'title', i.translations->'ko'->>'title', i.translations->'zh'->>'title'], null) as titles_translated,
                coalesce(ia.avatars, '{}') as avatars, coalesce(ic.cats, '{}') as cats, coalesce(ic.cats_primary, '{}') as cats_primary
         from items i
         left join (select item_id, array_agg(avatar_slug) as avatars from item_avatars group by item_id) ia on ia.item_id = i.id
         left join (select item_id, array_agg(category_slug) as cats, array_agg(category_slug) filter (where is_primary) as cats_primary
                    from item_categories group by item_id) ic on ic.item_id = i.id
         order by i.id",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let mut categories: Vec<(String, bool)> = Vec::new();
            let mut add = |slug: &str, primary: bool| match categories.iter_mut().find(|(s, _)| s == slug) {
                Some(e) => e.1 |= primary,
                None => categories.push((slug.to_owned(), primary)),
            };
            for slug in &r.cats {
                let primary = r.cats_primary.contains(slug);
                add(slug, primary);
                if let Some(p) = parent.get(slug) {
                    add(p, primary);
                }
            }
            let mut smart_tags: Vec<String> = r.booth_tags.iter().filter_map(|t| alias.get(&t.to_lowercase()).cloned()).collect();
            smart_tags.sort();
            smart_tags.dedup();
            let mut features = r.features;
            if r.quest.as_deref() == Some("Yes") {
                features.push("quest".into());
            }
            let titles_translated = r.titles_translated.into_iter().filter(|t| *t != r.title).collect();
            DocInput {
                id: r.id,
                bio: r.bio,
                title: r.title,
                titles_translated,
                author: r.author_name,
                booth_category: r.booth_category,
                price: r.price,
                likes: r.likes,
                published: r.published,
                is_adult: r.is_adult,
                rating: r.content_rating,
                features,
                avatars: r.avatars,
                categories,
                booth_tags: r.booth_tags,
                smart_tags,
                supported: r.supported,
                gender: r.gender,
            }
        })
        .collect())
}

/// Full item payloads for one result page, in the given order.
pub async fn load_items(pool: &PgPool, ids: &[i64]) -> Result<Vec<ItemResponse>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(i64, sqlx::types::Json<ItemResponse>)> = sqlx::query_as(
        "select i.id, jsonb_build_object(
            'id', i.id,
            'title', i.title,
            'bio', i.bio,
            'author_name', i.author_name,
            'author_uri', i.author_uri,
            'author_icon', i.author_icon,
            'booth_category', i.booth_category,
            'price_jpy', i.price_jpy,
            'smart_price', i.smart_price,
            'smart_price_reason', i.smart_price_reason,
            'likes', i.likes,
            'is_adult', i.is_adult,
            'content_rating', i.content_rating,
            'quest', i.quest,
            'features', to_jsonb(i.features),
            'status', i.status,
            'published_at', i.published_at,
            'scraped_at', i.scraped_at,
            'original_language', i.original_language,
            'avatar_slug', i.avatar_slug,
            'supported', i.supported,
            'gender', i.gender,
            'needs_enrichment', i.needs_enrichment,
            'last_listed_at', i.last_listed_at,
            'enrich_failures', i.enrich_failures,
            'media', to_jsonb(i.media),
            'booth_tags', to_jsonb(i.booth_tags),
            'variations', i.variations,
            'translations', i.translations,
            'linked_avatars', coalesce((select jsonb_agg(jsonb_build_object('slug', a.slug, 'en', a.name_en, 'ja', a.name_ja, 'ko', a.name_ko, 'image_url', coalesce((select b.images[1] from booth_images b where b.item_id = a.avatar_item_id), a.image_url)) order by a.slug)
                               from item_avatars ia join avatars a on a.slug = ia.avatar_slug where ia.item_id = i.id), '[]'::jsonb),
            'categories', coalesce((select jsonb_agg(jsonb_build_object('slug', ic.category_slug, 'is_primary', ic.is_primary))
                               from item_categories ic where ic.item_id = i.id), '[]'::jsonb),
            -- Booth image URLs (canonical); `media` keeps the archived VRCatalogue CDN URLs for delisted items
            'booth_images', (select to_jsonb(b.images) from booth_images b where b.item_id = i.id),
            'booth_images_complete', (select b.complete from booth_images b where b.item_id = i.id))
         from items i where i.id = any($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    let by_id: HashMap<i64, ItemResponse> = rows.into_iter().map(|(id, r)| (id, r.0)).collect();
    Ok(order_items(ids, by_id))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::*;

    #[test]
    fn exact_allowed_top_level_keys_and_no_embedding() {
        let item = ItemResponse {
            id: 12345,
            title: "Test Item".to_owned(),
            bio: "A nice item".to_owned(),
            author_name: "Test Author".to_owned(),
            author_uri: "https://author.example".to_owned(),
            author_icon: Some("https://author.example/icon.jpg".to_owned()),
            booth_category: Some("3d-clothing".to_owned()),
            price_jpy: Some(1500),
            smart_price: Some(1200),
            smart_price_reason: Some("Sale".to_owned()),
            likes: 42,
            is_adult: false,
            content_rating: Some("SFW".to_owned()),
            quest: Some("Yes".to_owned()),
            features: vec!["liltoon".to_owned(), "ma".to_owned()],
            status: "ok".to_owned(),
            published_at: Some(Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap()),
            scraped_at: Utc.with_ymd_and_hms(2026, 1, 15, 13, 0, 0).unwrap(),
            original_language: Some("ja".to_owned()),
            avatar_slug: Some("manuka".to_owned()),
            supported: true,
            gender: Some("female".to_owned()),
            needs_enrichment: false,
            last_listed_at: Some(Utc.with_ymd_and_hms(2026, 1, 16, 0, 0, 0).unwrap()),
            enrich_failures: 0,
            media: vec!["https://img.example/1.webp".to_owned()],
            booth_tags: vec!["VRChat".to_owned()],
            variations: json!([{"id": "v1", "name": "Default", "price_jpy": 1500, "status": "ok", "type": "standard"}]),
            translations: json!({"en": {"title": "Test Item EN"}}),
            linked_avatars: json!([{"slug": "manuka", "en": "Manuka", "image_url": null}]),
            categories: json!([{"slug": "clothing", "is_primary": true}]),
            booth_images: Some(vec!["https://booth.example/1.jpg".to_owned()]),
            booth_images_complete: Some(true),
        };

        let val = serde_json::to_value(&item).expect("serialization succeeds");
        let map = val.as_object().expect("serializes to JSON object");
        let keys: BTreeSet<String> = map.keys().cloned().collect();

        // 1. embedding and future/internal columns MUST NOT appear
        assert!(!keys.contains("embedding"), "embedding vector must not be exposed");
        assert_eq!(keys.len(), 33, "must have exactly 33 known business fields");

        let expected_keys: BTreeSet<String> = [
            "id", "title", "bio", "author_name", "author_uri", "author_icon",
            "booth_category", "price_jpy", "smart_price", "smart_price_reason",
            "likes", "is_adult", "content_rating", "quest", "features", "status",
            "published_at", "scraped_at", "original_language", "avatar_slug",
            "supported", "gender", "needs_enrichment", "last_listed_at",
            "enrich_failures", "media", "booth_tags", "variations", "translations",
            "linked_avatars", "categories", "booth_images", "booth_images_complete",
        ].iter().map(|&s| s.to_owned()).collect();

        assert_eq!(keys, expected_keys);
    }

    #[test]
    fn nonappearance_of_embedding_and_unknown_fields_from_raw_source() {
        let raw = json!({
            "id": 99,
            "title": "Item with vector",
            "embedding": [0.123, 0.456, 0.789],
            "future_internal_column": "secret",
            "features": ["ma"],
            "media": []
        });

        let item: ItemResponse = serde_json::from_value(raw).expect("deserializes safely");
        let reserialized = serde_json::to_value(&item).expect("serializes safely");
        let obj = reserialized.as_object().expect("object");

        assert!(!obj.contains_key("embedding"), "embedding omitted on reserialization");
        assert!(!obj.contains_key("future_internal_column"), "unknown fields omitted on reserialization");
        assert_eq!(obj.get("id").and_then(Value::as_i64), Some(99));
        assert_eq!(obj.get("title").and_then(Value::as_str), Some("Item with vector"));
    }

    #[test]
    fn retention_of_nullable_and_empty_business_data() {
        let item = ItemResponse {
            id: 1,
            author_icon: None,
            price_jpy: None,
            smart_price: None,
            booth_images: None,
            booth_images_complete: None,
            features: vec![],
            media: vec![],
            booth_tags: vec![],
            variations: json!([]),
            translations: json!({}),
            linked_avatars: json!([]),
            categories: json!([]),
            ..Default::default()
        };

        let val = serde_json::to_value(&item).unwrap();
        assert_eq!(val.get("author_icon"), Some(&Value::Null));
        assert_eq!(val.get("price_jpy"), Some(&Value::Null));
        assert_eq!(val.get("booth_images"), Some(&Value::Null));
        assert_eq!(val.get("booth_images_complete"), Some(&Value::Null));
        assert_eq!(val["features"], json!([]));
        assert_eq!(val["media"], json!([]));
        assert_eq!(val["variations"], json!([]));
        assert_eq!(val["translations"], json!({}));
    }

    #[test]
    fn order_preservation_and_empty_behavior() {
        let mut by_id = HashMap::new();
        by_id.insert(10, ItemResponse { id: 10, title: "Ten".into(), ..Default::default() });
        by_id.insert(20, ItemResponse { id: 20, title: "Twenty".into(), ..Default::default() });
        by_id.insert(30, ItemResponse { id: 30, title: "Thirty".into(), ..Default::default() });

        // Preserves exact query order
        let ordered = order_items(&[30, 10, 20], by_id.clone());
        let ids: Vec<i64> = ordered.iter().map(|it| it.id).collect();
        assert_eq!(ids, vec![30, 10, 20]);

        // Empty IDs
        let empty = order_items(&[], by_id.clone());
        assert!(empty.is_empty());

        // Drops missing IDs without error
        let filtered = order_items(&[20, 999, 10], by_id);
        let ids: Vec<i64> = filtered.iter().map(|it| it.id).collect();
        assert_eq!(ids, vec![20, 10]);
    }
}
