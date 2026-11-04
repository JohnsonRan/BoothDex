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
#[serde(deny_unknown_fields)] // a misspelt key in load_items' SQL fails loudly instead of leaving an Option as None
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
    /// delisted, but re-uploaded by the shop under this id
    pub relisted_as: Option<i64>,
}

#[cfg(test)]
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
            relisted_as: None,
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
    relisted: bool,
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

    // only titles are translated (titles_translated); descriptions are searched in their original language.
    // One jsonpath call detoasts `translations` (~2 KB, TOASTed) once per row instead of once per `->` (≈1.5x faster); silent mode keeps non-object translations from failing the load.
    let rows: Vec<Row> = sqlx::query_as(
        "select i.id, i.title, i.bio, i.author_name, i.booth_category, coalesce(i.smart_price, i.price_jpy) as price, i.likes,
                extract(epoch from i.published_at)::bigint as published, i.is_adult, i.content_rating, i.quest, i.features,
                i.booth_tags, i.supported, i.gender,
                array(select jsonb_array_elements_text(jsonb_path_query_array(i.translations,
                    '$.keyvalue() ? (@.key == \"en\" || @.key == \"ja\" || @.key == \"ko\" || @.key == \"zh\").value.title ? (@ != null)', '{}', true))) as titles_translated,
                coalesce(ia.avatars, '{}') as avatars, coalesce(ic.cats, '{}') as cats, coalesce(ic.cats_primary, '{}') as cats_primary,
                i.status = 'delisted' and i.relisted_as is not null as relisted
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
                relisted: r.relisted,
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
            'booth_images_complete', (select b.complete from booth_images b where b.item_id = i.id),
            'relisted_as', i.relisted_as)
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
    use std::collections::HashMap;

    use super::*;

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
