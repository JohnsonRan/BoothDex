//! Run with a disposable pgvector database: DATABASE_URL=... cargo test -p api -- --ignored.

use super::*;

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn semantic_search_uses_vectors_with_and_without_a_named_model(pool: PgPool) {
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap().block_on(check_semantic_search(pool));
    }).await.unwrap();
}

async fn check_semantic_search(pool: PgPool) {
    use axum::body::to_bytes;
    use axum::http::HeaderMap;

    for (key, expected) in [("AI_BASE_URL", "http://127.0.0.1:9"), ("AI_API_KEY", "test"), ("AI_EMBED_MODEL", "test-model")] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty() || v.trim() == expected), "unset {key} or use its local test value");
    }
    let mut vector = vec![0.0; catalogue_core::embed::DIMS];
    vector[0] = 1.0;
    for (id, title, x, y) in [(1i64, "ボブカット", 1.0, 0.0), (2, "ドレス", 0.0, 1.0), (3, "ヘアセット", 1.0, 0.1)] {
        let mut v = vec![0.0; catalogue_core::embed::DIMS];
        (v[0], v[1]) = (x, y);
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_category, embedding)
                     values ($1, $2, 'creator', 'https://example.test', false, 'ok', now(), '3D Clothing', $3::halfvec)")
            .bind(id).bind(title).bind(catalogue_core::embed::to_pg(&v)).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("insert into avatars (slug, name_en) values ('shinano', 'Shinano'), ('manuka', 'Manuka') on conflict (slug) do nothing;
                   insert into item_avatars (item_id, avatar_slug) values (1, 'shinano'), (2, 'shinano'), (3, 'manuka');
                   insert into app_settings (key, value) values ('ai', '{\"base_url\":\"http://127.0.0.1:9\",\"api_key\":\"test\",\"embed_model\":\"test-model\"}');")
        .execute(&pool).await.unwrap();
    sqlx::query("insert into query_embeddings (model, query, embedding) values ('test-model', 'silky strands', $1::halfvec)")
        .bind(catalogue_core::embed::to_pg(&vector)).execute(&pool).await.unwrap();

    let catalog = Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap();
    let state = Arc::new(AppState {
        pool,
        snap: arc_swap::ArcSwap::from_pointee(Snapshot {
            catalog,
            category_phrases: Default::default(),
            avatars: Lookup::new(vec![(vec!["shinano".into()], json!({ "slug": "shinano" }), 2)]),
            authors: Lookup::new(Vec::new()),
            tags: Lookup::new(Vec::new()),
            booth_tags: Lookup::new(Vec::new()),
            categories: json!({ "categories": [] }),
            version: 1,
            loaded_at: chrono::Utc::now(),
            build_secs: 0.0,
        }),
        reload: tokio::sync::Notify::new(),
        site_url: "http://127.0.0.1:3000".into(),
        throttle: Default::default(),
        dummy_hash: String::new(),
        gallery: gallery::Fetcher::new().unwrap(),
        sys: admin::SysMetrics::new(),
        http: reqwest::Client::builder().timeout(std::time::Duration::from_secs(2)).build().unwrap(),
        imgproxy_url: None,
        limits: Default::default(),
        search_permits: tokio::sync::Semaphore::new(16),
        embed_cache: Default::default(),
        embed_inflight: Default::default(),
        embed_permits: Arc::new(tokio::sync::Semaphore::new(4)),
    });
    for (query, expected, semantic) in [
        ("q=silky%20strands", vec![1, 3], "used"),
        ("q=shinano%20silky%20strands", vec![1], "used"),
        ("q=silky%20strands&sem=0", vec![], "skipped"),
    ] {
        let response = search(State(state.clone()), limit::ClientIp("203.0.113.20".parse().unwrap()), HeaderMap::new(), RawQuery(Some(query.into())))
            .await.unwrap_or_else(IntoResponse::into_response);
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
        assert_eq!(body["semantic"], semantic, "{body}");
        let ids: Vec<i64> = body["items"].as_array().unwrap().iter().map(|item| item["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, expected, "query {query}");
        assert_eq!(body["count"], expected.len());
    }
}
