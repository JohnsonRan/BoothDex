//! Run with a disposable pgvector database: DATABASE_URL=... cargo test -p api -- --ignored.

use std::collections::HashMap;

use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

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
    app_with(pool, catalog, avatars, Default::default(), 2)
}

/// `http_secs`: the timeout of the API's outgoing requests, the embedding provider's included.
fn app_with(pool: PgPool, catalog: Catalog, avatars: Lookup, related: crate::related::Related, http_secs: u64) -> Arc<AppState> {
    crate::install_tls();
    Arc::new(
        AppState::new(
            pool,
            Snapshot {
                catalog,
                category_phrases: Default::default(),
                related,
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
            Some(std::time::Duration::from_secs(http_secs)),
        )
        .unwrap(),
    )
}

/// Embeddings from a provider at a closed local port, so a search that asks for one fails at once and reaches no
/// upstream; refuses to run when the environment names another provider.
async fn closed_provider(pool: &PgPool) {
    for (key, expected) in [("AI_BASE_URL", "http://127.0.0.1:9"), ("AI_API_KEY", "test"), ("AI_EMBED_MODEL", "test-model")] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty() || v.trim() == expected), "unset {key} or use its local test value");
    }
    sqlx::query("insert into app_settings (key, value) values ('ai', '{\"base_url\":\"http://127.0.0.1:9\",\"api_key\":\"test\",\"embed_model\":\"test-model\"}')")
        .execute(pool).await.unwrap();
}

async fn check_semantic_search(pool: PgPool) {
    use axum::body::to_bytes;
    use axum::http::HeaderMap;

    closed_provider(&pool).await;
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
                   insert into item_avatars (item_id, avatar_slug) values (1, 'shinano'), (2, 'shinano'), (3, 'manuka');")
        .execute(&pool).await.unwrap();
    sqlx::query("insert into query_embeddings (model, query, embedding) values ('test-model', 'silky strands', $1::halfvec)")
        .bind(boothdex_core::embed::HalfVector::from_f32_slice(&vector)).execute(&pool).await.unwrap();

    let catalog = Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap();
    let state = app(pool, catalog, Lookup::new(vec![(vec!["shinano".into()], json!({ "slug": "shinano" }), 2)]));
    let shinano = |w: &str| json!([{ "kind": "model", "slug": "shinano", "words": [w] }]);
    // a misspelt model name is read as the model, and the site is told a typo was allowed; typo=0 takes the words
    // as typed, while a name spelt right still counts
    for (query, expected, semantic, recognized, typo) in [
        ("q=silky%20strands", vec![1, 3], "used", json!([]), false),
        ("q=Shinano%20silky%20strands", vec![1], "used", shinano("Shinano"), false),
        ("q=Shinnano%20silky%20strands", vec![1], "used", shinano("Shinnano"), true),
        ("q=Shinnano%20silky%20strands&typo=0", vec![], "unavailable", json!([]), false),
        ("q=Shinano%20silky%20strands&typo=0", vec![1], "used", shinano("Shinano"), false),
        ("q=silky%20strands&sem=0", vec![], "skipped", json!([]), false),
    ] {
        let response = search(State(state.clone()), limit::ClientIp("203.0.113.20".parse().unwrap()), crate::limit::Tier::Base, HeaderMap::new(), RawQuery(Some(query.into())))
            .await.unwrap_or_else(IntoResponse::into_response);
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
        assert_eq!(body["semantic"], semantic, "{body}");
        assert_eq!(body["recognized"], recognized, "{body}");
        assert_eq!((&body["typoTolerant"], &body["nameTypo"]), (&json!(typo), &json!(typo)), "{body}");
        let ids: Vec<i64> = body["items"].as_array().unwrap().iter().map(|item| item["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, expected, "query {query}");
        assert_eq!(body["count"], expected.len());
    }

    // an empty search answers, in the same response, what it finds without each filter group; the meaning-based
    // matches of the request carry over (the provider at 127.0.0.1:9 never answers, the stored embedding does)
    let empty = "q=silky%20strands&avatars=shinano&pmin=100000&without=avatars+pmin";
    let response = search(State(state.clone()), limit::ClientIp("203.0.113.21".parse().unwrap()), crate::limit::Tier::Base, HeaderMap::new(), RawQuery(Some(empty.into())))
        .await
        .unwrap_or_else(IntoResponse::into_response);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!((&body["count"], &body["semantic"], &body["widerCounts"], &body["typoTolerant"]), (&json!(0), &json!("used"), &json!([0, 1]), &json!(false)), "{body}");
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_slow_query_embedding_is_pending_and_the_next_search_uses_it(pool: PgPool) {
    multi_thread(check_pending_embedding(pool)).await;
}

async fn check_pending_embedding(pool: PgPool) {
    use std::sync::atomic::{AtomicUsize, Ordering};

    for key in ["AI_BASE_URL", "AI_API_KEY", "AI_EMBED_MODEL"] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}: this test's provider is a local stand-in");
    }
    let mut vector = vec![0.0; boothdex_core::embed::DIMS];
    vector[0] = 1.0;
    sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, embedding)
                 values (1, 'ボブカット', 'creator', 'https://example.test', false, 'ok', now(), $1::halfvec)")
        .bind(boothdex_core::embed::HalfVector::from_f32_slice(&vector)).execute(&pool).await.unwrap();
    // a local stand-in provider that answers after 2.8 s, past the search's 2.5 s wait
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let reply = json!({ "data": [{ "index": 0, "embedding": vector }] });
    let provider = axum::Router::new().fallback(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        let reply = reply.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(2800)).await;
            axum::Json(reply)
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    sqlx::query("insert into app_settings (key, value) values ('ai', $1)")
        .bind(json!({ "base_url": base_url, "api_key": "test", "embed_model": "test-model" }))
        .execute(&pool).await.unwrap();
    let catalog = Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap();
    let state = app_with(pool, catalog, Lookup::new(Vec::new()), Default::default(), 10);

    // the first search answers with words only while the provider works on; the same search within the provider's
    // time waits for that request instead of sending another, and uses its vector
    let first = search_json(&state, "q=silky%20strands").await;
    assert_eq!(first["semantic"], "pending", "{first}");
    let second = search_json(&state, "q=silky%20strands").await;
    assert_eq!(second["semantic"], "used", "{second}");
    assert_eq!(second["items"][0]["id"], 1, "{second}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // with every provider slot taken, a search that waits in vain is unavailable, not pending, and asks nothing
    let held = state.embed_permits.clone().acquire_many_owned(4).await.unwrap();
    let no_slot = search_json(&state, "q=wavy%20bangs").await;
    assert_eq!(no_slot["semantic"], "unavailable", "{no_slot}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    // so is a search the address's embedding budget refuses
    while state.limits.embed_ip.take("203.0.113.40".parse().unwrap()).is_ok() {}
    let refused = search_json(&state, "q=curly%20bangs").await;
    assert_eq!(refused["semantic"], "unavailable", "{refused}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(held);
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
    let found = palette(State(state), ip, crate::limit::Tier::Base, RawQuery(Some("q=Rock%26Roll".into()))).await.unwrap_or_else(IntoResponse::into_response);
    let json: Value = serde_json::from_slice(&body(found).await).unwrap();
    assert_eq!(json["items"].as_array().unwrap().iter().map(|i| i["id"].as_i64().unwrap()).collect::<Vec<_>>(), [1]);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_search_also_finds_what_the_tag_translating_it_finds(pool: PgPool) {
    multi_thread(check_expansion(pool)).await;
}

async fn search_json(state: &Arc<AppState>, query: &str) -> Value {
    use axum::body::to_bytes;
    use axum::http::HeaderMap;

    let response = search(State(state.clone()), limit::ClientIp("203.0.113.40".parse().unwrap()), crate::limit::Tier::Base, HeaderMap::new(), RawQuery(Some(query.into())))
        .await
        .unwrap_or_else(IntoResponse::into_response);
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

async fn check_expansion(pool: PgPool) {
    use axum::body::to_bytes;

    // 1-12 are titled スカート with the title translated as Skirt; 13-22, in another BOOTH category, carry only the tag
    sqlx::raw_sql(
        "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_category, booth_tags, translations)
         select i, 'スカート ' || i, 'creator', 'https://example.test', false, 'ok', now(), '3D Clothing', '{スカート}',
                jsonb_build_object('en', jsonb_build_object('title', 'Skirt ' || i))
         from generate_series(1, 12) i;
         insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_category, booth_tags)
         select i, 'Item ' || i, 'creator', 'https://example.test', false, 'ok', now(), '3D Accessory', '{スカート}'
         from generate_series(13, 22) i;
         insert into avatars (slug, name_en) values ('shinano', 'Shinano') on conflict (slug) do nothing;
         insert into item_avatars (item_id, avatar_slug) values (1, 'shinano'), (13, 'shinano');",
    )
    .execute(&pool)
    .await
    .unwrap();
    let avatars = Lookup::new(vec![(vec!["shinano".into()], json!({ "slug": "shinano" }), 2)]);
    let state = app(pool.clone(), Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap(), avatars);
    let ids = |body: &Value| body["items"].as_array().unwrap().iter().map(|i| i["id"].as_i64().unwrap()).collect::<Vec<_>>();

    // sem=0: no embedding is asked for. The words' own matches rank first, the tag's follow and are counted.
    let both = search_json(&state, "q=skirt&sem=0&pageSize=100").await;
    assert_eq!((&both["count"], &both["expandedWith"]), (&json!(22), &json!(["スカート"])), "{both}");
    assert_eq!(both["boothCounts"], json!({ "3D Clothing": 12, "3D Accessory": 10 }));
    let mut first = ids(&both)[..12].to_vec();
    first.sort();
    assert_eq!(first, (1..=12).collect::<Vec<_>>());
    let words = search_json(&state, "q=skirt&sem=0&xl=0&pageSize=100").await;
    assert_eq!((&words["count"], &words["expandedWith"], &words["boothCounts"]), (&json!(12), &json!([]), &json!({ "3D Clothing": 12 })), "{words}");
    // a model named in the query: the other words are translated, and what the tag finds must fit the model
    let fitting = search_json(&state, "q=shinano%20skirt&sem=0&pageSize=100").await;
    assert_eq!((ids(&fitting), &fitting["expandedWith"]), (vec![1, 13], &json!(["スカート"])), "{fitting}");
    // an empty search counts what it finds without its filter from the same matches
    for (query, wider) in [("q=skirt&sem=0&pmin=100000&without=pmin", 22), ("q=skirt&sem=0&xl=0&pmin=100000&without=pmin", 12)] {
        let body = search_json(&state, query).await;
        assert_eq!((&body["count"], &body["widerCounts"], &body["expandedWith"]), (&json!(0), &json!([wider]), &json!([])), "{query}: {body}");
    }

    // the feed and the palette search the words alone
    let ip = limit::ClientIp("203.0.113.41".parse().unwrap());
    let feed = rss_feed(State(state.clone()), ip, RawQuery(Some("q=skirt".into()))).await.unwrap_or_else(IntoResponse::into_response);
    let xml = String::from_utf8(to_bytes(feed.into_body(), 1024 * 1024).await.unwrap().to_vec()).unwrap();
    assert_eq!(xml.matches("<item>").count(), 12);
    let found = palette(State(state), ip, crate::limit::Tier::Base, RawQuery(Some("q=skirt".into()))).await.unwrap_or_else(IntoResponse::into_response);
    let json: Value = serde_json::from_slice(&to_bytes(found.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!(json["itemCount"], 12);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_booth_link_or_an_item_number_finds_the_item_without_an_embedding(pool: PgPool) {
    multi_thread(check_item_reference(pool)).await;
}

async fn check_item_reference(pool: PgPool) {
    use axum::body::to_bytes;

    // 70001 is re-uploaded as 70002. 1-12, titled in English with its number, share the tag Bundle with 13-22, so
    // the number alone would also be searched as that tag.
    closed_provider(&pool).await;
    sqlx::raw_sql(
        "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, relisted_as) values
         (70001, 'Ribbon', 'creator', 'https://example.test', false, 'delisted', now(), 70002),
         (70002, 'Ribbon', 'creator', 'https://example.test', false, 'ok', now(), null);
         insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, original_language, booth_tags)
         select i, 'Bundle for 70001 #' || i, 'creator', 'https://example.test', false, 'ok', now(), 'en', '{Bundle}'
         from generate_series(1, 12) i;
         insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags)
         select i, 'Item ' || i, 'creator', 'https://example.test', false, 'ok', now(), '{Bundle}' from generate_series(13, 22) i;",
    )
    .execute(&pool)
    .await
    .unwrap();
    let state = app(pool.clone(), Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap(), Lookup::new(Vec::new()));
    let ids = |body: &Value| body["items"].as_array().unwrap().iter().map(|i| i["id"].as_i64().unwrap()).collect::<Vec<_>>();
    let asked_for_embedding = |body: &Value| body["trace"].as_array().unwrap().iter().any(|s| s["step"] == "settings");

    // the named item first, then what the number finds as text; no embedding is asked for and no tag searched
    let number = search_json(&state, "q=70001&pageSize=100").await;
    assert_eq!((&number["count"], &number["semantic"], &number["expandedWith"]), (&json!(13), &json!("skipped"), &json!([])), "{number}");
    assert_eq!(ids(&number)[0], 70001);
    assert!(!asked_for_embedding(&number), "{number}");
    for link in ["https://booth.pm/ko/items/70001", "https://shop.booth.pm/items/70001/"] {
        let body = search_json(&state, &format!("q={link}")).await;
        assert_eq!((ids(&body), &body["semantic"]), (vec![70001], &json!("skipped")), "{link}: {body}");
        assert!(!asked_for_embedding(&body), "{body}");
    }
    // a link to an item the catalogue lacks finds nothing, and is not embedded either
    let missing = search_json(&state, "q=https://booth.pm/ja/items/99999").await;
    assert_eq!((&missing["count"], &missing["semantic"]), (&json!(0), &json!("skipped")), "{missing}");
    // a number that is no item's id is searched as today: the embedding is asked for (the closed port refuses it)
    let text = search_json(&state, "q=99999").await;
    assert_eq!((&text["count"], &text["semantic"]), (&json!(0), &json!("unavailable")), "{text}");
    assert!(asked_for_embedding(&text), "{text}");

    // the palette lists it first too
    let ip = limit::ClientIp("203.0.113.42".parse().unwrap());
    for q in ["70001", "https://booth.pm/ja/items/70001"] {
        let found = palette(State(state.clone()), ip, crate::limit::Tier::Base, RawQuery(Some(format!("q={q}")))).await.unwrap_or_else(IntoResponse::into_response);
        let json: Value = serde_json::from_slice(&to_bytes(found.into_body(), 1024 * 1024).await.unwrap()).unwrap();
        assert_eq!(json["items"][0]["id"], 70001, "{q}: {json}");
    }
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn related_searches_come_from_a_stored_query_embedding_only(pool: PgPool) {
    multi_thread(check_related(pool)).await;
}

async fn check_related(pool: PgPool) {
    use axum::body::to_bytes;
    use axum::http::HeaderMap;

    use crate::related::{AvatarNames, Related, TagCentroids};

    // 1-20 are long skirts that fit Shinano, 21-40 pleated skirts, from five shops each and embedded near the query;
    // 41-440 are untagged, so neither tag is on 5% of the items
    closed_provider(&pool).await;
    let unit = |i: usize, x: f32| {
        let mut v = vec![0.0; boothdex_core::embed::DIMS];
        (v[0], v[i]) = (x, (1.0 - x * x).sqrt());
        boothdex_core::embed::HalfVector::from_f32_slice(&v)
    };
    for id in 1..=40i64 {
        let (tag, near) = if id <= 20 { ("long skirt", unit(1, 0.9)) } else { ("pleats", unit(2, 0.8)) };
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags, embedding)
                     values ($1, 'Skirt ' || $1, 'shop ' || ($1 % 5), 'https://example.test', false, 'ok', now(), array[$2], $3::halfvec)")
            .bind(id).bind(tag).bind(near).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql(
        "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at)
         select i, 'Filler ' || i, 'shop ' || (i % 50), 'https://example.test', false, 'ok', now() from generate_series(41, 440) i;
         insert into avatars (slug, name_en) values ('shinano', 'Shinano') on conflict (slug) do nothing;
         insert into item_avatars (item_id, avatar_slug) select i, 'shinano' from generate_series(1, 20) i;
         insert into app_settings (key, value) values ('embed_state', '{\"model\": \"test-model\"}');",
    )
    .execute(&pool)
    .await
    .unwrap();
    // stored for skirt, and for an item link as if a search had embedded it before links named items
    for query in ["skirt", "https://booth.pm/ja/items/1"] {
        sqlx::query("insert into query_embeddings (model, query, embedding) values ('test-model', $1, $2::halfvec)")
            .bind(query).bind(unit(3, 1.0)).execute(&pool).await.unwrap();
    }
    let catalog = Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap();
    let centroids = TagCentroids::current(&pool, &catalog, None).await.unwrap();
    let related = Related { avatars: AvatarNames::new(["Shinano"]), centroids };
    let state = app_with(pool.clone(), catalog, Lookup::new(vec![(vec!["shinano".into()], json!({ "slug": "shinano" }), 20)]), related, 2);
    let ip = "203.0.113.50".parse().unwrap();
    let used = || sqlx::query_as::<_, (i32, chrono::DateTime<chrono::Utc>)>("select hits, last_used_at from query_embeddings where query = 'skirt'").fetch_one(&pool);
    let before = used().await.unwrap();
    let searches = |query: &'static str| {
        let state = state.clone();
        async move {
            let response = crate::related::related(State(state), limit::ClientIp(ip), HeaderMap::new(), RawQuery(Some(query.into())))
                .await
                .unwrap_or_else(IntoResponse::into_response);
            assert_eq!(response.status(), StatusCode::OK, "{query}");
            let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
            body["searches"].clone()
        }
    };

    // nearest first; a model named in the query stays in each destination, which finds only what fits it
    assert_eq!(searches("q=skirt").await, json!([{ "label": "long skirt", "q": "long skirt" }, { "label": "pleats", "q": "pleats" }]));
    assert_eq!(searches("q=Shinano%20skirt").await, json!([{ "label": "long skirt", "q": "Shinano long skirt" }]));
    // a misspelt model name is read as the model unless typo=0 asks for the words as typed
    assert_eq!(searches("q=Shinnano%20skirt").await, json!([{ "label": "long skirt", "q": "Shinnano long skirt" }]));
    assert_eq!(searches("q=Shinnano%20skirt&typo=0").await, json!([]));
    // the embedding a search keeps in memory counts as stored
    let mut kept = vec![0.0; boothdex_core::embed::DIMS];
    kept[0] = 1.0;
    state.embed_cache.insert("test-model\ncoat".into(), kept);
    assert_eq!(searches("q=coat").await, json!([{ "label": "long skirt", "q": "long skirt" }, { "label": "pleats", "q": "pleats" }]));
    // reading the stored embedding counts no use
    assert_eq!(used().await.unwrap(), before);
    // none for a query without a stored embedding, which is not asked for: no row is stored and the embedding budget
    // of the address is whole
    assert_eq!(searches("q=dress").await, json!([]));
    let stored: i64 = sqlx::query_scalar("select count(*) from query_embeddings").fetch_one(&pool).await.unwrap();
    assert_eq!(stored, 2);
    assert!((0..8).all(|_| state.limits.embed_ip.take(ip).is_ok()));
    // none for an item named by its link, for favourites while signed out, or without words
    for query in ["q=https://booth.pm/ja/items/1", "q=skirt&v=favourites", "q=Shinano", "adult=include"] {
        assert_eq!(searches(query).await, json!([]), "{query}");
    }
    // none once the configured model is not the one of the item embeddings, though searches stored vectors from it,
    // or no model is configured
    sqlx::query("insert into query_embeddings (model, query, embedding) values ('other-model', 'skirt', $1::halfvec)")
        .bind(unit(3, 1.0)).execute(&pool).await.unwrap();
    let config = (*state.ai_config().await.unwrap()).clone();
    for model in [Some("other-model".to_owned()), None] {
        state.ai.store(Some(Arc::new(boothdex_core::settings::AiConfig { embed_model: model.clone(), ..config.clone() })));
        assert_eq!(searches("q=skirt").await, json!([]), "{model:?}");
    }
    // the work waits for a search permit like a search, and gives up after its wait as a search does
    state.ai.store(Some(Arc::new(config)));
    let _held = state.search_permits.acquire_many(16).await.unwrap();
    let response = crate::related::related(State(state.clone()), limit::ClientIp(ip), HeaderMap::new(), RawQuery(Some("q=skirt".into())))
        .await
        .unwrap_or_else(IntoResponse::into_response);
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn the_palette_items_follow_the_callers_adult_setting(pool: PgPool) {
    multi_thread(check_palette_adult(pool)).await;
}

async fn check_palette_adult(pool: PgPool) {
    use axum::body::to_bytes;

    sqlx::raw_sql("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, published_at) values
                   (1, 'Rock Band Tee', 'creator', 'https://example.test', false, 'ok', now(), now()),
                   (2, 'Rock Adult Set', 'creator', 'https://example.test', true, 'ok', now(), now());")
        .execute(&pool).await.unwrap();
    let state = app(pool.clone(), Catalog::build(db::load_docs(&pool).await.unwrap(), 0, None).unwrap(), Lookup::new(Vec::new()));
    let ip = limit::ClientIp("203.0.113.31".parse().unwrap());
    for (query, expected) in [("q=rock", vec![1]), ("q=rock&adult=adult_only", vec![2]), ("q=rock&adult=include", vec![1, 2])] {
        let found = palette(State(state.clone()), ip, crate::limit::Tier::Base, RawQuery(Some(query.into()))).await.unwrap_or_else(IntoResponse::into_response);
        let json: Value = serde_json::from_slice(&to_bytes(found.into_body(), 1024 * 1024).await.unwrap()).unwrap();
        let mut ids: Vec<i64> = json["items"].as_array().unwrap().iter().map(|i| i["id"].as_i64().unwrap()).collect();
        ids.sort();
        assert_eq!(ids, expected, "query {query}");
    }
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
         (4, 'Original 4', 'creator', 'https://example.test', false, 'ok', now(), '42');
         insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, original_language, translations) values
         (5, '原标题', 'creator', 'https://example.test', false, 'ok', now(), 'zh', '{\"en\":{\"title\":\"Title EN 5\"},\"zh\":{\"title\":\"标题\"}}');",
    )
    .execute(&pool)
    .await
    .unwrap();

    let docs = db::load_docs(&pool).await.unwrap();
    let languages: HashMap<i64, u8> = docs.iter().map(|d| (d.id, d.title_languages)).collect();
    let zh: HashMap<i64, (Option<String>, bool)> = docs.iter().map(|d| (d.id, (d.title_zh.clone(), d.title_is_zh))).collect();
    let by_id: HashMap<i64, Vec<String>> = docs.into_iter().map(|d| (d.id, d.titles_translated)).collect();

    assert_eq!(*by_id.get(&1).unwrap(), vec!["Title EN", "Title JA", "Title KO"]);
    assert_eq!(*by_id.get(&2).unwrap(), vec!["Title EN 2"]);
    assert!(by_id.get(&3).unwrap().is_empty());
    assert!(by_id.get(&4).unwrap().is_empty());
    // the Chinese title apart, searched as Chinese; a Chinese listing's own title, whatever its translation says
    let chinese = |title: Option<&str>, own: bool| (title.map(str::to_owned), own);
    assert_eq!([1, 2, 3, 5].map(|id| zh[&id].clone()), [chinese(Some("Title ZH"), false), chinese(Some("Title ZH 2"), false), chinese(None, false), chinese(Some("原标题"), true)]);
    // the languages it has a title in (expansion::LANGUAGES: en, ja, ko, zh), a null title not counted, and the
    // original language too
    assert_eq!([1, 2, 3, 4, 5].map(|id| languages[&id]), [0b1111, 0b1001, 0, 0, 0b1001]);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn search_finds_items_by_the_words_of_the_tag_dictionary(pool: PgPool) {
    sqlx::raw_sql(
        "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags) values
         (1, 'Item 1', 'creator', 'https://example.test', false, 'ok', now(), '{テクスチャ}'),
         (2, 'Item 2', 'creator', 'https://example.test', false, 'ok', now(), '{椅子}'),
         (3, 'Item 3', 'creator', 'https://example.test', false, 'ok', now(), '{Hair Clip}');
         insert into booth_tag_translations (tag, en, ja, ko, zh, keywords, source) values
         ('テクスチャ', 'texture', 'テクスチャ', null, '纹理', '{贴图}', 'import'),
         ('椅子', 'chair', '椅子', '의자', '椅子', '{}', 'localize');",
    )
    .execute(&pool)
    .await
    .unwrap();
    let words = db::load_tag_words(&pool).await.unwrap();
    assert_eq!(words.len(), 2);
    let catalog = Catalog::build_with_tag_words(db::load_docs(&pool).await.unwrap(), &words, 0, None).unwrap();
    let found = |q: &str| {
        let request = crate::search_request::SearchRequest::from(&crate::params::Params::parse(&format!("q={q}")));
        let mut ids = catalog.search(&request, 0, None, None, None, false).unwrap().ids;
        ids.sort();
        ids
    };
    // a translation and a keyword find the item; "hair" finds the tag that has the word, not the one translated "chair"
    assert_eq!([found("texture"), found("贴图"), found("chair"), found("의자"), found("hair")], [vec![1], vec![1], vec![2], vec![2], vec![3]]);
}

#[test]
fn nearest_binds_only_parameters_used_by_each_query() {
    use sqlx::{Arguments, Execute};

    for models in [vec![], vec!["shinano".to_owned()]] {
        let mut query = super::search::nearest_query(&[0.5; boothdex_core::embed::DIMS], &models);
        assert_eq!(query.take_arguments().unwrap().unwrap().len(), if models.is_empty() { 1 } else { 2 });
        assert_eq!(query.sql().as_str().contains("$2"), !models.is_empty());
    }
}

#[test]
fn queries_are_normalized_for_the_cache() {
    assert_eq!(super::search::normalize_query("  Gothic   Dress "), "gothic dress");
    // query_embeddings cannot store a NUL
    assert_eq!(super::search::normalize_query("gothic\0dress\0"), "gothic dress");
}

#[test]
fn semantic_cutoff_follows_the_best_match_and_drops_weak_ones() {
    // values seen during calibration: a relevant query, and gibberish whose best neighbour is still 0.707
    assert!((super::search::semantic_cutoff(0.787, false) - 0.727).abs() < 1e-9);
    assert!(0.707 < super::search::semantic_cutoff(0.707, false));
    assert!(0.707 < super::search::semantic_cutoff(0.707, true));
    assert!((super::search::semantic_cutoff(0.830, true) - 0.730).abs() < 1e-9);
}

#[test]
fn queries_naming_a_category_find_it() {
    let cats = serde_json::json!([
        {"slug": "hair", "en": "Hair", "ja": "髪型", "children": [
            {"slug": "hair--short", "en": "Short", "ja": "ショート"}, {"slug": "hair--other", "en": "Other"},
            {"slug": "hair--kemomimi", "en": "Animal ears", "ja": "けもみみ"}]},
        {"slug": "clothing", "en": "Clothing", "children": [{"slug": "clothing--other", "en": "Other"}]}
    ]);
    let m = super::snapshot::category_phrases(&cats);
    for q in ["shorthair", "hairshort", "short", "ショート"] {
        assert_eq!(m.get(q).map(String::as_str), Some("hair--short"), "{q}");
    }
    // the search folds the query the same way (search.rs)
    for q in ["しょーと", "ＳＨＯＲＴ", "ｼｮｰﾄ"] {
        assert_eq!(m.get(&super::text::fold(q)).map(String::as_str), Some("hair--short"), "{q}");
    }
    assert_eq!(m.get(&super::text::fold("ケモミミ")).map(String::as_str), Some("hair--kemomimi"));
    assert_eq!(m.get("hair").map(String::as_str), Some("hair"));
    // a child name two parents share only counts together with its parent
    assert_eq!(m.get("other"), None);
    assert_eq!(m.get("otherclothing").map(String::as_str), Some("clothing--other"));
}
