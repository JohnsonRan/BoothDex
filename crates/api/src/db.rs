use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;

use crate::catalog::DocInput;

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    title: String,
    titles_translated: Vec<String>,
    bio: String,
    bio_en: Option<String>,
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

    let rows: Vec<Row> = sqlx::query_as(
        "select i.id, i.title, i.bio, i.author_name, i.booth_category, coalesce(i.smart_price, i.price_jpy) as price, i.likes,
                extract(epoch from i.published_at)::bigint as published, i.is_adult, i.content_rating, i.quest, i.features,
                i.booth_tags, i.supported, i.gender, i.translations->'en'->>'bio' as bio_en,
                array_remove(array[i.translations->'en'->>'title', i.translations->'ja'->>'title', i.translations->'ko'->>'title'], null) as titles_translated,
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
                bio: match r.bio_en {
                    Some(en) if en != r.bio => format!("{}\n{en}", r.bio),
                    _ => r.bio,
                },
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
pub async fn load_items(pool: &PgPool, ids: &[i64]) -> Result<Vec<Value>> {
    let rows: Vec<(i64, Value)> = sqlx::query_as(
        "select i.id, to_jsonb(i) || jsonb_build_object(
            'linked_avatars', coalesce((select jsonb_agg(jsonb_build_object('slug', a.slug, 'en', a.name_en, 'ja', a.name_ja, 'ko', a.name_ko, 'image_url', a.image_url) order by a.slug)
                               from item_avatars ia join avatars a on a.slug = ia.avatar_slug where ia.item_id = i.id), '[]'),
            'categories', coalesce((select jsonb_agg(jsonb_build_object('slug', ic.category_slug, 'is_primary', ic.is_primary))
                               from item_categories ic where ic.item_id = i.id), '[]'),
            -- Booth image URLs (canonical); `media` keeps the archived VRCatalogue CDN URLs for delisted items
            'booth_images', (select to_jsonb(b.images) from booth_images b where b.item_id = i.id),
            'booth_images_complete', (select b.complete from booth_images b where b.item_id = i.id))
         from items i where i.id = any($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    let mut by_id: HashMap<i64, Value> = rows.into_iter().collect();
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}
