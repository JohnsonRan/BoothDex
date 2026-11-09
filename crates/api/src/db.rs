use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::catalog::DocInput;

/// Explicit typed representation of an item in API responses. The site's TypeScript types are generated from
/// this and the types below (see `api_types`), so a change here that the site doesn't follow fails `tsc`.
///
/// Internal columns like `embedding` (vectors) are deliberately omitted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
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
    pub likes: i32,
    pub is_adult: bool,
    pub content_rating: Option<ContentRating>,
    pub quest: Option<String>,
    pub features: Vec<String>,
    pub status: Status,
    pub published_at: Option<DateTime<Utc>>,
    pub scraped_at: DateTime<Utc>,
    pub original_language: Option<String>,
    pub avatar_slug: Option<String>,
    pub supported: bool,
    pub gender: Option<Gender>,
    pub needs_enrichment: bool,
    pub last_listed_at: Option<DateTime<Utc>>,
    pub enrich_failures: i16,
    pub media: Vec<String>,
    pub booth_tags: Vec<String>,
    pub variations: Vec<Variation>,
    /// AI translations by language, plus `original_language`. Kept as raw JSON: a few rows carry stray keys, and
    /// the API passes them through untouched. The site sees the documented shape.
    #[cfg_attr(test, ts(as = "Translations"))]
    pub translations: Value,
    pub linked_avatars: Vec<LinkedAvatar>,
    pub categories: Vec<ItemCategory>,
    pub booth_images: Option<Vec<String>>,
    pub booth_images_complete: Option<bool>,
    /// delisted, but re-uploaded by the shop under this id
    pub relisted_as: Option<i64>,
}

// Values the database allows (CHECK constraints on `items`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ContentRating {
    #[allow(clippy::upper_case_acronyms)]
    SFW,
    Suggestive,
    #[allow(clippy::upper_case_acronyms)]
    NSFW,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Ok,
    Delisted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Gender {
    Female,
    Male,
}

/// One purchasable variation, as stored by the crawler (`boothdex_core::booth::item_fields`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Variation {
    pub id: VariationId,
    pub name: String,
    pub price_jpy: Option<i64>,
    pub status: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// The crawler stores Booth's id as a string; items imported from VRCatalogue have numbers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(untagged)]
pub enum VariationId {
    Number(i64),
    Text(String),
}

/// A model the item fits (built in `load_items`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LinkedAvatar {
    pub slug: String,
    pub en: String,
    pub ja: Option<String>,
    pub ko: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ItemCategory {
    pub slug: String,
    pub is_primary: bool,
}

/// The documented shape of `ItemResponse::translations`, for the generated TypeScript only.
#[cfg(test)]
#[derive(ts_rs::TS)]
#[ts(optional_fields)]
#[allow(dead_code)]
pub struct Translations {
    zh: Option<Translation>,
    en: Option<Translation>,
    ja: Option<Translation>,
    ko: Option<Translation>,
    original_language: Option<String>,
}

#[cfg(test)]
#[derive(ts_rs::TS)]
#[ts(optional_fields)]
#[allow(dead_code)]
pub struct Translation {
    title: Option<String>,
    bio: Option<String>,
    /// variation name -> translated name (null when the model returned none)
    variations: Option<std::collections::BTreeMap<String, Option<String>>>,
}

#[cfg(test)]
impl ItemResponse {
    /// Reference projection for checking the SQL card path: bio, variations, tags, linked avatars and categories come
    /// back empty, translations keep only titles and images only the cover and hover preview.
    /// A page of 100 shrinks from ~1.3 MB to ~100 KB of JSON; the item page loads the full record.
    pub fn card(mut self) -> Self {
        self.bio.clear();
        self.booth_tags.clear();
        self.variations.clear();
        self.linked_avatars.clear();
        self.categories.clear();
        self.media.truncate(2);
        if let Some(b) = &mut self.booth_images {
            b.truncate(2);
        }
        let titles = match &self.translations {
            Value::Object(langs) => langs
                .iter()
                .filter_map(|(lang, t)| Some((lang.clone(), serde_json::json!({ "title": t.get("title")? }))))
                .collect(),
            _ => serde_json::Map::new(),
        };
        self.translations = Value::Object(titles);
        self
    }
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
            likes: 0,
            is_adult: false,
            content_rating: None,
            quest: None,
            features: Vec::new(),
            status: Status::Ok,
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
            variations: Vec::new(),
            translations: serde_json::json!({}),
            linked_avatars: Vec::new(),
            categories: Vec::new(),
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

/// Item payloads in the given order. Card mode avoids loading details that result grids discard.
pub async fn load_items(pool: &PgPool, ids: &[i64], card: bool) -> Result<Vec<ItemResponse>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(i64, sqlx::types::Json<ItemResponse>)> = sqlx::query_as(
        "select i.id, jsonb_build_object(
            'id', i.id,
            'title', i.title,
            'bio', case when $2 then '' else i.bio end,
            'author_name', i.author_name,
            'author_uri', i.author_uri,
            'author_icon', i.author_icon,
            'booth_category', i.booth_category,
            'price_jpy', i.price_jpy,
            'smart_price', i.smart_price,
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
            'media', to_jsonb(case when $2 then i.media[1:2] else i.media end),
            'booth_tags', case when $2 then '[]'::jsonb else to_jsonb(i.booth_tags) end,
            'variations', case when $2 then '[]'::jsonb else i.variations end,
            'translations', case when $2 then (
                select coalesce(jsonb_object_agg(lang, jsonb_build_object('title', translation->'title')), '{}'::jsonb)
                from jsonb_each(case when jsonb_typeof(i.translations) = 'object' then i.translations else '{}'::jsonb end) t(lang, translation)
                where jsonb_typeof(translation) = 'object' and translation ? 'title'
            ) else i.translations end,
            'linked_avatars', case when $2 then '[]'::jsonb else coalesce((select jsonb_agg(jsonb_build_object('slug', a.slug, 'en', a.name_en, 'ja', a.name_ja, 'ko', a.name_ko, 'image_url', coalesce((select b.images[1] from booth_images b where b.item_id = a.avatar_item_id), a.image_url)) order by a.slug)
                               from item_avatars ia join avatars a on a.slug = ia.avatar_slug where ia.item_id = i.id), '[]'::jsonb) end,
            'categories', case when $2 then '[]'::jsonb else coalesce((select jsonb_agg(jsonb_build_object('slug', ic.category_slug, 'is_primary', ic.is_primary))
                               from item_categories ic where ic.item_id = i.id), '[]'::jsonb) end,
            -- Booth image URLs (canonical); `media` keeps the archived VRCatalogue CDN URLs for delisted items
            'booth_images', (select to_jsonb(case when $2 then b.images[1:2] else b.images end) from booth_images b where b.item_id = i.id),
            'booth_images_complete', (select b.complete from booth_images b where b.item_id = i.id),
            'relisted_as', i.relisted_as)
         from items i where i.id = any($1)",
    )
    .bind(ids)
    .bind(card)
    .fetch_all(pool)
    .await?;
    let by_id: HashMap<i64, ItemResponse> = rows.into_iter().map(|(id, r)| (id, r.0)).collect();
    Ok(order_items(ids, by_id))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;

    #[test]
    fn card_keeps_what_a_result_card_shows() {
        let full = ItemResponse {
            id: 1,
            title: "Title".into(),
            bio: "Long description".into(),
            booth_tags: vec!["tag".into()],
            media: vec!["m1".into(), "m2".into(), "m3".into()],
            booth_images: Some(vec!["b1".into(), "b2".into(), "b3".into(), "b4".into()]),
            variations: vec![Variation {
                id: VariationId::Text("v".into()),
                name: "A".into(),
                price_jpy: Some(500),
                status: "addable_to_cart".into(),
                kind: "digital".into(),
            }],
            linked_avatars: vec![LinkedAvatar { slug: "shinano".into(), en: "Shinano".into(), ja: None, ko: None, image_url: None }],
            categories: vec![ItemCategory { slug: "hair".into(), is_primary: true }],
            translations: json!({
                "en": { "title": "T", "bio": "B", "variations": { "v": "V" } },
                "ja": { "bio": "only a bio" },
                "original_language": "ja",
            }),
            ..Default::default()
        };
        let card = full.card();
        assert_eq!(card.title, "Title");
        assert_eq!(card.bio, "");
        assert!(card.booth_tags.is_empty());
        assert_eq!(card.media, vec!["m1", "m2"]);
        assert_eq!(card.booth_images, Some(vec!["b1".into(), "b2".into()]));
        assert!(card.variations.is_empty() && card.linked_avatars.is_empty() && card.categories.is_empty());
        assert_eq!(card.translations, json!({ "en": { "title": "T" } }));
    }

    #[test]
    fn card_tolerates_non_object_translations() {
        let card = ItemResponse { translations: json!(null), ..Default::default() }.card();
        assert_eq!(card.translations, json!({}));
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
