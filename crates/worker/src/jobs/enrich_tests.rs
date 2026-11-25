use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sqlx::PgPool;

use super::Ctx;
use super::enrich::{ENRICH_ITEMS_SELECT, enrich};
use crate::booth::Booth;

/// Local stand-in for the AI provider in enrich tests: records every request body and answers with `answer`.
async fn enrich_stand_in(pool: &PgPool, answer: impl Fn(&Value) -> String + Send + Sync + 'static) -> Arc<Mutex<Vec<Value>>> {
    for key in ["AI_BASE_URL", "AI_API_KEY", "AI_MODEL"] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}: this test's provider is a local stand-in");
    }
    crate::install_tls();
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen = requests.clone();
    let answer = Arc::new(answer);
    let app = axum::Router::new().fallback(move |axum::Json(body): axum::Json<Value>| {
        seen.lock().unwrap().push(body.clone());
        let content = answer(&body);
        async move { axum::Json(json!({"choices": [{"message": {"content": content}}]})) }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    sqlx::query("insert into app_settings (key, value) values ('ai', $1)")
        .bind(json!({"base_url": base_url, "model": "stand-in", "api_key": "test"}))
        .execute(pool)
        .await
        .unwrap();
    requests
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn enrich_rows_read_into_the_typed_item(pool: PgPool) {
    sqlx::query(
        "insert into items (id, title, bio, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment, enrich_failures, booth_category, price_jpy, booth_tags, variations, translations)
         values (1, 'Ring', 'desc', 'c', 'https://example.test', false, 'ok', now(), true, 0, '3D', 500, '{VRChat,MA}',
                 '[{\"id\":\"1\",\"name\":\"Full set\",\"price_jpy\":500},{\"id\":\"2\",\"name\":\"\",\"price_jpy\":null}]', '{\"en\":{\"title\":\"Ring\"}}'),
                (2, 'Bare', '', 'c', 'https://example.test', false, 'ok', now(), true, 2, null, null, '{}', '[]', '{}'),
                (3, 'Failed thrice', '', 'c', 'https://example.test', false, 'ok', now(), true, 3, null, null, '{}', '[]', '{}'),
                (4, 'Done', '', 'c', 'https://example.test', false, 'ok', now(), false, 0, null, null, '{}', '[]', '{}')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let items: Vec<crate::ai::EnrichItem> = sqlx::query_as(ENRICH_ITEMS_SELECT).bind(10i64).bind(boothdex_core::ENRICH_ATTEMPTS).fetch_all(&pool).await.unwrap();
    assert_eq!(items.iter().map(|i| i.id).collect::<Vec<_>>(), [2, 1]);
    let ring = &items[1];
    assert_eq!(ring.match_text(), "Ring VRChat MA Full set ");
    let prompt = crate::ai::enrich_prompt(ring, "cats", &[]);
    assert!(prompt.contains("Listed price: ¥500\nTags: VRChat, MA\nVariations:\n- Full set (¥500)\n-  (¥null)"), "{prompt}");
    let bare = crate::ai::enrich_prompt(&items[0], "cats", &[]);
    assert!(bare.contains("Booth category: \nListed price: ¥null\nTags: \nVariations:\n(none)"), "{bare}");
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn enrich_names_the_listings_avatars_and_keeps_them_in_titles(pool: PgPool) {
    let titles = json!({
        "en": {"title": "[For Manuka] Frill Dress"},
        "ja": {"title": "【マヌカ対応】フリルドレス"},
        "ko": {"title": "【マヌカ 대응】프릴 드레스"},
        "zh": {"title": "【マヌカ対応】荷叶边连衣裙"},
    });
    let reply = json!({
        "categories": ["clothing--outfit-dresses"], "content_rating": "SFW", "quest_compatible": "No", "smart_price": 1500,
        "features": [], "translations": titles, "original_language": "ja",
    });
    let requests = enrich_stand_in(&pool, move |_| reply.to_string()).await;
    sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment, booth_tags) values (1, '【マヌカ対応】フリルドレス', 'c', 'https://example.test', false, 'ok', now(), true, '{ドレス,VRChat}')")
        .execute(&pool).await.unwrap();
    let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'running', now()) returning id").fetch_one(&pool).await.unwrap();
    crate::relink::record_current(&pool).await;
    let booth = Booth::new().unwrap();
    assert_eq!(enrich(&mut Ctx::new(&pool, &booth, job), &json!({})).await.unwrap(), "1 enriched, 0 failed of 1");

    let requests = requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1);
    let system = requests[0]["messages"][0]["content"].as_str().unwrap();
    for rule in ["never translated or transliterated", "虚拟形象 for an avatar", "3D模型 for a 3D model file", "贴图 for a texture", "法线贴图 for a normal map", "材质 for a material", "着色器 for a shader", "网格 for a mesh"] {
        assert!(system.contains(rule), "{rule}: {system}");
    }
    let user = requests[0]["messages"][1]["content"].as_str().unwrap();
    assert!(user.contains("\nAvatar names (Japanese / romaji): マヌカ / Manuka\nDescription:"), "{user}");
    let (translations, avatars): (Value, Vec<String>) =
        sqlx::query_as("select translations, array(select avatar_slug from item_avatars where item_id = 1) from items where id = 1").fetch_one(&pool).await.unwrap();
    let mut expected = titles.clone();
    expected["original_language"] = json!("ja");
    assert_eq!(translations, expected);
    assert_eq!(avatars, ["manuka"]);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn enrich_corrects_failing_titles_once_and_writes_only_those_that_pass(pool: PgPool) {
    let reply = |en: &str, zh: &str| {
        json!({
            "categories": ["clothing--outfit-dresses"], "content_rating": "SFW", "quest_compatible": "No", "smart_price": 1500, "features": [],
            "translations": {"en": {"title": en}, "ja": {"title": "【マヌカ対応】フリルドレス"}, "ko": {"title": "【マヌカ 대응】프릴 드레스"}, "zh": {"title": zh}},
            "original_language": "ja",
        })
    };
    let requests = enrich_stand_in(&pool, move |body| {
        let listing = body["messages"][1]["content"].as_str().unwrap();
        let item = if listing.contains("Title: 【マヌカ対応】フリルドレス2") { 2 } else if listing.contains("Title: 【マヌカ対応】リボン") { 3 } else { 1 };
        let correction = body["messages"].as_array().unwrap().len() == 4;
        match (item, correction) {
            (1, false) => reply("[For the avatar] Frill Dress 3", "【マヌカ対応】荷叶边ドレス").to_string(),
            (1, true) => reply("[For Manuka] Frill Dress", "【マヌカ対応】荷叶边ドレス").to_string(),
            (2, false) => reply("[For Manuka] Frill Dress 2", "【マヌカ対応】荷叶边ドレス2").to_string(),
            (2, true) => json!({"categories": []}).to_string(),
            (_, false) => reply("[For Manuka] Ribbon", "【マヌカ対応】リボン").to_string(),
            (_, true) => "Sorry, I can't help with that.".to_owned(),
        }
    })
    .await;
    sqlx::query(
        "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment, booth_tags, translations)
         values (1, '【マヌカ対応】フリルドレス', 'c', 'https://example.test', false, 'ok', now(), true, '{ドレス}', '{}'),
                (2, '【マヌカ対応】フリルドレス2', 'c', 'https://example.test', false, 'ok', now(), true, '{ドレス}', '{\"zh\": {\"title\": \"【マヌカ対応】荷叶边连衣裙2\"}}'),
                (3, '【マヌカ対応】リボン', 'c', 'https://example.test', false, 'ok', now(), true, '{リボン}', '{}')",
    )
    .execute(&pool).await.unwrap();
    let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'running', now()) returning id").fetch_one(&pool).await.unwrap();
    crate::relink::record_current(&pool).await;
    let booth = Booth::new().unwrap();
    assert_eq!(enrich(&mut Ctx::new(&pool, &booth, job), &json!({"concurrency": 1})).await.unwrap(), "3 enriched, 0 failed of 3");

    let requests = requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 6);
    let fixes: Vec<&str> = requests.iter().filter_map(|r| r["messages"].get(3).and_then(|m| m["content"].as_str())).collect();
    assert!(fixes.iter().any(|f| f.contains("translations.en.title drops the avatar name manuka") && f.contains("translations.zh.title holds kana")), "{fixes:?}");
    let titles = |id: i64| {
        let pool = &pool;
        async move { sqlx::query_scalar::<_, Value>("select translations from items where id = $1").bind(id).fetch_one(pool).await.unwrap() }
    };
    assert_eq!(titles(1).await, json!({"en": {"title": "[For Manuka] Frill Dress"}, "ja": {"title": "【マヌカ対応】フリルドレス"}, "ko": {"title": "【マヌカ 대응】프릴 드레스"}, "original_language": "ja"}));
    assert_eq!(titles(2).await["zh"], json!({"title": "【マヌカ対応】荷叶边连衣裙2"}));
    assert_eq!(titles(2).await["en"], json!({"title": "[For Manuka] Frill Dress 2"}));
    assert_eq!(titles(3).await, json!({"en": {"title": "[For Manuka] Ribbon"}, "ja": {"title": "【マヌカ対応】フリルドレス"}, "ko": {"title": "【マヌカ 대응】프릴 드레스"}, "original_language": "ja"}));
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn enrich_links_items_to_added_avatar_names_before_any_ai_request(pool: PgPool) {
    for key in ["AI_BASE_URL", "AI_MODEL", "AI_API_KEY"] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}: this test must reach no provider");
    }
    crate::install_tls();
    crate::relink::record_current(&pool).await;
    sqlx::query("update avatars set aliases = aliases || '{ぬぺぽ}' where slug = 'manuka'").execute(&pool).await.unwrap();
    sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment) values (1, '【ぬぺぽ対応】服', 's', 'u', false, 'ok', now(), true), (2, 'nothing', 's', 'u', false, 'ok', now(), true)")
        .execute(&pool).await.unwrap();
    let booth = Booth::new().unwrap();
    let enrich_job = |params: Value| {
        let (pool, booth) = (&pool, &booth);
        async move {
            sqlx::query("update jobs set status = 'succeeded' where status = 'running'").execute(pool).await.unwrap();
            let id: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'running', now()) returning id").fetch_one(pool).await.unwrap();
            enrich(&mut Ctx::new(pool, booth, id), &params).await.unwrap()
        }
    };
    let linked = || async { sqlx::query_as::<_, (i64, String)>("select item_id, avatar_slug from item_avatars order by 1").fetch_all(&pool).await.unwrap() };
    sqlx::query("insert into app_settings (key, value) values ('ai', '{\"base_url\": \"http://127.0.0.1:9\", \"model\": \"m\", \"api_key\": \"k\"}')").execute(&pool).await.unwrap();
    assert_eq!(enrich_job(json!({ "relink_only": true })).await, "1 avatar links added");
    assert_eq!(linked().await, [(1, "manuka".to_owned())]);
    assert_eq!(enrich_job(json!({ "relink_only": true })).await, "no avatar names added");
    sqlx::query("delete from app_settings where key = 'ai'").execute(&pool).await.unwrap();
    sqlx::query("update avatars set aliases = aliases || '{nothing}' where slug = 'shinano'").execute(&pool).await.unwrap();
    assert_eq!(enrich_job(json!({})).await, "1 avatar links added; skipped: AI provider not configured");
    assert_eq!(linked().await, [(1, "manuka".to_owned()), (2, "shinano".to_owned())]);
    sqlx::query("update avatars set aliases = aliases || '{ぬぺ}' where slug = 'shinano'").execute(&pool).await.unwrap();
    sqlx::query("update jobs set status = 'succeeded' where status = 'running'").execute(&pool).await.unwrap();
    let id: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'cancelling', now()) returning id").fetch_one(&pool).await.unwrap();
    assert_eq!(enrich(&mut Ctx::new(&pool, &booth, id), &json!({})).await.unwrap(), "cancelled while linking avatars, 1 links added");
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn enrich_links_an_alias_without_holding_it_in_titles(pool: PgPool) {
    sqlx::query("update avatars set aliases = aliases || '{ぬぺぽ}' where slug = 'manuka'").execute(&pool).await.unwrap();
    crate::relink::record_current(&pool).await;
    let titles = json!({"en": {"title": "Frill Dress"}, "ja": {"title": "【ぬぺぽ対応】フリルドレス"}, "ko": {"title": "프릴 드레스"}, "zh": {"title": "荷叶边连衣裙"}});
    let reply = json!({"categories": ["clothing--outfit-dresses"], "content_rating": "SFW", "quest_compatible": "No", "smart_price": 1500, "features": [], "translations": titles, "original_language": "ja"});
    let requests = enrich_stand_in(&pool, move |_| reply.to_string()).await;
    sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment, booth_tags) values (1, '【ぬぺぽ対応】フリルドレス', 'c', 'https://example.test', false, 'ok', now(), true, '{ドレス}')")
        .execute(&pool).await.unwrap();
    let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'running', now()) returning id").fetch_one(&pool).await.unwrap();
    let booth = Booth::new().unwrap();
    assert_eq!(enrich(&mut Ctx::new(&pool, &booth, job), &json!({})).await.unwrap(), "1 enriched, 0 failed of 1");
    assert_eq!(requests.lock().unwrap().len(), 1);
    let (translations, avatars): (Value, Vec<String>) =
        sqlx::query_as("select translations, array(select avatar_slug from item_avatars where item_id = 1) from items where id = 1").fetch_one(&pool).await.unwrap();
    let mut expected = titles.clone();
    expected["original_language"] = json!("ja");
    assert_eq!(translations, expected);
    assert_eq!(avatars, ["manuka"]);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn enrich_links_items_to_the_names_migration_0032_added(pool: PgPool) {
    for key in ["AI_BASE_URL", "AI_MODEL", "AI_API_KEY"] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}: this test must reach no provider");
    }
    crate::install_tls();
    sqlx::query(
        "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags) values
           (1, 'ドレス', 's', 'u', false, 'ok', now(), '{薄荷}'),
           (2, '【クマリ対応】ワンピース', 's', 'u', false, 'ok', now(), '{}'),
           (3, 'ライム用 髪', 's', 'u', false, 'ok', now(), '{}')",
    )
    .execute(&pool).await.unwrap();
    let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'running', now()) returning id").fetch_one(&pool).await.unwrap();
    let booth = Booth::new().unwrap();
    assert_eq!(enrich(&mut Ctx::new(&pool, &booth, job), &json!({})).await.unwrap(), "2 avatar links added; skipped: AI provider not configured");
    let linked: Vec<(i64, String)> = sqlx::query_as("select item_id, avatar_slug from item_avatars order by 1").fetch_all(&pool).await.unwrap();
    assert_eq!(linked, [(1, "hakka".to_owned()), (2, "kumaly".to_owned())]);
    let pending: bool = sqlx::query_scalar(
        "select exists (select 1 from avatars a, unnest(array_remove(array[a.name_en, a.name_ja, a.name_ko], null) || a.aliases) n
                        where not coalesce((select avatar_names from catalog_meta) -> a.slug ? n, false))",
    )
    .fetch_one(&pool).await.unwrap();
    assert!(!pending);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn enrich_keeps_first_reply_passing_zh_when_correction_omits_it(pool: PgPool) {
    let requests = enrich_stand_in(&pool, |body| {
        let correction = body["messages"].as_array().unwrap().len() == 4;
        if !correction {
            json!({
                "categories": ["clothing--outfit-dresses"], "content_rating": "SFW", "quest_compatible": "No", "smart_price": 1500, "features": [],
                "translations": {
                    "en": {"title": "[For the avatar] Frill Dress"},
                    "ja": {"title": "【マヌカ対応】フリルドレス"},
                    "ko": {"title": "【マヌカ 대응】프릴 드레스"},
                    "zh": {"title": "【マヌカ対応】荷叶边连衣裙"},
                },
                "original_language": "ja",
            })
            .to_string()
        } else {
            json!({
                "categories": ["clothing--outfit-dresses"], "content_rating": "SFW", "quest_compatible": "No", "smart_price": 1500, "features": [],
                "translations": {
                    "en": {"title": "[For Manuka] Frill Dress"},
                    "ja": {"title": "【マヌカ対応】フリルドレス"},
                    "ko": {"title": "【マヌカ 대응】프릴 드레스"},
                },
                "original_language": "ja",
            })
            .to_string()
        }
    })
    .await;
    sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment, booth_tags) values (1, '【マヌカ対応】フリルドレス', 'c', 'https://example.test', false, 'ok', now(), true, '{ドレス}')")
        .execute(&pool)
        .await
        .unwrap();
    let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'running', now()) returning id").fetch_one(&pool).await.unwrap();
    crate::relink::record_current(&pool).await;
    let booth = Booth::new().unwrap();
    assert_eq!(enrich(&mut Ctx::new(&pool, &booth, job), &json!({"concurrency": 1})).await.unwrap(), "1 enriched, 0 failed of 1");
    assert_eq!(requests.lock().unwrap().len(), 2);
    let translations: Value = sqlx::query_scalar("select translations from items where id = 1").fetch_one(&pool).await.unwrap();
    assert_eq!(translations["en"], json!({"title": "[For Manuka] Frill Dress"}));
    assert_eq!(translations["zh"], json!({"title": "【マヌカ対応】荷叶边连衣裙"}));
}
