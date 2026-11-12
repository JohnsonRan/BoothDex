//! Run with a disposable pgvector database: DATABASE_URL=... cargo test -p api -- --ignored.

use super::*;
use crate::catalog::Catalog;
use crate::search::search;

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn semantic_search_uses_vectors_with_and_without_a_named_model(pool: PgPool) {
    multi_thread(check_semantic_search(pool)).await;
}

/// The handlers' `block_in_place` needs a multi-threaded runtime; `sqlx::test` gives a current-thread one.
async fn multi_thread(check: impl Future<Output = ()> + Send + 'static) {
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap().block_on(check);
    }).await.unwrap();
}

fn app(pool: PgPool, catalog: Catalog, avatars: Lookup) -> Arc<AppState> {
    crate::install_tls();
    Arc::new(
        AppState::new(
            pool,
            Snapshot {
                catalog,
                category_phrases: Default::default(),
                avatars,
                authors: Lookup::new(Vec::new()),
                tags: Lookup::new(Vec::new()),
                booth_tags: Lookup::new(Vec::new()),
                categories: json!({ "categories": [] }),
                version: 1,
                loaded_at: chrono::Utc::now(),
                build_secs: 0.0,
            },
            Default::default(),
            "http://127.0.0.1:3000".into(),
            None,
            String::new(),
            Some(std::time::Duration::from_secs(2)),
        )
        .unwrap(),
    )
}

async fn check_semantic_search(pool: PgPool) {
    use axum::body::to_bytes;
    use axum::http::HeaderMap;

    for (key, expected) in [("AI_BASE_URL", "http://127.0.0.1:9"), ("AI_API_KEY", "test"), ("AI_EMBED_MODEL", "test-model")] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty() || v.trim() == expected), "unset {key} or use its local test value");
    }
    let mut vector = vec![0.0; boothdex_core::embed::DIMS];
    vector[0] = 1.0;
    for (id, title, x, y) in [(1i64, "ボブカット", 1.0, 0.0), (2, "ドレス", 0.0, 1.0), (3, "ヘアセット", 1.0, 0.1)] {
        let mut v = vec![0.0; boothdex_core::embed::DIMS];
        (v[0], v[1]) = (x, y);
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_category, embedding)
                     values ($1, $2, 'creator', 'https://example.test', false, 'ok', now(), '3D Clothing', $3::halfvec)")
            .bind(id).bind(title).bind(boothdex_core::embed::HalfVector::from_f32_slice(&v)).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("insert into avatars (slug, name_en) values ('shinano', 'Shinano'), ('manuka', 'Manuka') on conflict (slug) do nothing;
                   insert into item_avatars (item_id, avatar_slug) values (1, 'shinano'), (2, 'shinano'), (3, 'manuka');
                   insert into app_settings (key, value) values ('ai', '{\"base_url\":\"http://127.0.0.1:9\",\"api_key\":\"test\",\"embed_model\":\"test-model\"}');")
        .execute(&pool).await.unwrap();
    sqlx::query("insert into query_embeddings (model, query, embedding) values ('test-model', 'silky strands', $1::halfvec)")
        .bind(boothdex_core::embed::HalfVector::from_f32_slice(&vector)).execute(&pool).await.unwrap();

    let catalog = Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap();
    let state = app(pool, catalog, Lookup::new(vec![(vec!["shinano".into()], json!({ "slug": "shinano" }), 2)]));
    let shinano = json!([{ "kind": "model", "slug": "shinano", "words": ["Shinano"] }]);
    for (query, expected, semantic, recognized) in [
        ("q=silky%20strands", vec![1, 3], "used", json!([])),
        ("q=Shinano%20silky%20strands", vec![1], "used", shinano),
        ("q=silky%20strands&sem=0", vec![], "skipped", json!([])),
    ] {
        let response = search(State(state.clone()), limit::ClientIp("203.0.113.20".parse().unwrap()), HeaderMap::new(), RawQuery(Some(query.into())))
            .await.unwrap_or_else(IntoResponse::into_response);
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
        assert_eq!(body["semantic"], semantic, "{body}");
        assert_eq!(body["recognized"], recognized, "{body}");
        let ids: Vec<i64> = body["items"].as_array().unwrap().iter().map(|item| item["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, expected, "query {query}");
        assert_eq!(body["count"], expected.len());
    }
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn the_feed_pins_its_order_and_the_palette_searches_the_text_as_typed(pool: PgPool) {
    multi_thread(check_feed_and_palette(pool)).await;
}

async fn check_feed_and_palette(pool: PgPool) {
    use axum::body::to_bytes;

    sqlx::raw_sql("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, published_at, price_jpy) values
                   (1, 'Rock&Roll Hoodie', 'creator', 'https://example.test', false, 'ok', now(), now() - interval '2 days', 300),
                   (2, 'Rock Band Tee', 'creator', 'https://example.test', false, 'ok', now(), now() - interval '1 day', 100),
                   (3, 'Plain Socks', 'creator', 'https://example.test', false, 'ok', now(), now() - interval '3 days', 200);")
        .execute(&pool).await.unwrap();
    let state = app(pool.clone(), Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap(), Lookup::new(Vec::new()));
    let ip = limit::ClientIp("203.0.113.30".parse().unwrap());
    let body = |r: Response| async { to_bytes(r.into_body(), 1024 * 1024).await.unwrap() };

    // a saved search's own order and page size do not change the feed: newest first, all of them
    let feed = rss_feed(State(state.clone()), ip, RawQuery(Some("sort=price&dir=asc&pageSize=1&page=2".into()))).await.unwrap_or_else(IntoResponse::into_response);
    let xml = String::from_utf8(body(feed).await.to_vec()).unwrap();
    let titles: Vec<&str> = xml.split("<item><title>").skip(1).map(|t| t.split('<').next().unwrap()).collect();
    assert_eq!(titles, ["Rock Band Tee", "Rock&amp;Roll Hoodie", "Plain Socks"]);

    // an `&` in the text stays part of it: both words must match, so the other "Rock" item is left out
    let found = palette(State(state), ip, RawQuery(Some("q=Rock%26Roll".into()))).await.unwrap_or_else(IntoResponse::into_response);
    let json: Value = serde_json::from_slice(&body(found).await).unwrap();
    assert_eq!(json["items"].as_array().unwrap().iter().map(|i| i["id"].as_i64().unwrap()).collect::<Vec<_>>(), [1]);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn load_items_fills_every_field(pool: PgPool) {
    // every column set: a key missing from load_items' jsonb_build_object would come back as null (Option = None)
    sqlx::raw_sql(
        "insert into avatars (slug, name_en) values ('shinano', 'Shinano') on conflict (slug) do nothing;
         insert into items (id, title, bio, author_name, author_uri, author_icon, booth_category, price_jpy, smart_price,
                            likes, is_adult, content_rating, quest, features, status, published_at,
                            scraped_at, original_language, media, booth_tags, variations, translations, avatar_slug,
                            supported, gender, needs_enrichment, last_listed_at, enrich_failures, relisted_as)
         values (1, 'Title', 'Bio', 'creator', 'https://example.test', 'https://example.test/i.png', '3D Clothing', 500, 400,
                 3, false, 'SFW', 'Yes', '{PhysBones}', 'delisted', now(), now(), 'ja', '{https://example.test/m.png}',
                 '{tag}', '[]', '{}', 'shinano', true, 'female', false, now(), 1, 2);
         insert into item_avatars (item_id, avatar_slug) values (1, 'shinano');
         insert into item_categories (item_id, category_slug, is_primary) select 1, slug, true from smart_categories limit 1;
         insert into booth_images (item_id, images, complete, fetched_at) values (1, '{https://example.test/b.png}', true, now());",
    )
    .execute(&pool)
    .await
    .unwrap();

    let items = db::load_items(&pool, &[1], false).await.unwrap();
    assert_eq!(db::load_items(&pool, &[1], true).await.unwrap(), items.iter().cloned().map(db::ItemResponse::card).collect::<Vec<_>>());
    let item = serde_json::to_value(&items[0]).unwrap();
    let nulls: Vec<&String> = item.as_object().unwrap().iter().filter(|(_, v)| v.is_null()).map(|(k, _)| k).collect();
    assert!(nulls.is_empty(), "load_items left these fields null: {nulls:?}");
    assert_eq!(item["linked_avatars"][0]["slug"], "shinano");
    assert_eq!(item["categories"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn sql_cards_match_the_reference_projection(pool: PgPool) {
    let translations = [
        json!({ "en": { "title": "Title", "bio": "Long bio", "variations": { "v": "V" } }, "ja": { "title": null }, "ko": { "bio": "No title" }, "original_language": "ja" }),
        json!({ "en": null, "ja": "text", "ko": [], "zh": { "title": "中文" }, "custom": { "title": 42 } }),
        json!(null), json!(42), json!(["array"]), json!({}),
    ];
    for (i, translation) in translations.iter().enumerate() {
        sqlx::query(
            "insert into items (id, title, bio, author_name, author_uri, is_adult, status, scraped_at, media, booth_tags, variations, translations)
             values ($1, 'Title', repeat('Description ', 1000), 'creator', 'https://example.test', false, 'ok', now(), '{m1,m2,m3}', '{tag}', '[{\"id\":1,\"name\":\"Variation\",\"price_jpy\":500,\"status\":\"available\",\"type\":\"digital\"}]', $2)",
        )
        .bind(i as i64 + 1)
        .bind(translation)
        .execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("insert into booth_images (item_id, images, complete, fetched_at) values (1, '{b1,b2,b3}', true, now()), (2, '{}', false, now());
                   update items set media = '{}' where id = 2;").execute(&pool).await.unwrap();
    let ids = [6, 1, 2, 3, 999, 4, 5];
    let full = db::load_items(&pool, &ids, false).await.unwrap();
    let cards = db::load_items(&pool, &ids, true).await.unwrap();
    assert_eq!(cards, full.into_iter().map(db::ItemResponse::card).collect::<Vec<_>>());
    assert!(db::load_items(&pool, &[], true).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn load_docs_translated_titles(pool: PgPool) {
    sqlx::raw_sql(
        "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, translations) values
         (1, 'Original 1', 'creator', 'https://example.test', false, 'ok', now(), '{\"original_language\":\"ja\",\"en\":{\"title\":\"Title EN\"},\"ja\":{\"title\":\"Title JA\"},\"ko\":{\"title\":\"Title KO\"},\"zh\":{\"title\":\"Title ZH\"}}'),
         (2, 'Original 2', 'creator', 'https://example.test', false, 'ok', now(), '{\"en\":{\"title\":\"Title EN 2\"},\"ja\":{\"title\":null},\"ko\":{},\"zh\":{\"title\":\"Title ZH 2\"}}'),
         (3, 'Original 3', 'creator', 'https://example.test', false, 'ok', now(), '{\"fr\":{\"title\":\"Titre FR\"}}'),
         (4, 'Original 4', 'creator', 'https://example.test', false, 'ok', now(), '42');",
    )
    .execute(&pool)
    .await
    .unwrap();

    let docs = db::load_docs(&pool).await.unwrap();
    let by_id: HashMap<i64, Vec<String>> = docs.into_iter().map(|d| (d.id, d.titles_translated)).collect();

    assert_eq!(*by_id.get(&1).unwrap(), vec!["Title EN", "Title JA", "Title KO", "Title ZH"]);
    assert_eq!(*by_id.get(&2).unwrap(), vec!["Title EN 2", "Title ZH 2"]);
    assert!(by_id.get(&3).unwrap().is_empty());
    assert!(by_id.get(&4).unwrap().is_empty());
}
