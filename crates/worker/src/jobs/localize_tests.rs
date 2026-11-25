use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sqlx::PgPool;

use super::Ctx;
use super::localize::localize;
use crate::booth::Booth;

/// What the stand-in provider answers a tag request (its body, the asked tags in id order): the reply's content, or
/// a refusal's status and body.
pub(super) type TagAnswer = std::result::Result<String, (u16, String)>;

/// A local stand-in for the AI provider: a taxonomy request gets "中 " before each English name but those starting
/// "omit:", a tag request what `answer` gives. Returns every request body it got.
pub(super) async fn stand_in_provider(pool: &PgPool, answer: impl Fn(&Value, &[String]) -> TagAnswer + Send + Sync + 'static) -> Arc<Mutex<Vec<Value>>> {
    use axum::response::IntoResponse;
    for key in ["AI_BASE_URL", "AI_API_KEY", "AI_MODEL"] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}: this test's provider is a local stand-in");
    }
    crate::install_tls();
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen = requests.clone();
    let answer = Arc::new(answer);
    let app = axum::Router::new().fallback(move |axum::Json(body): axum::Json<Value>| {
        seen.lock().unwrap().push(body.clone());
        let reply = if body["messages"][0]["content"] == crate::ai::TAG_SYSTEM {
            answer(&body, &asked_in(&body))
        } else {
            let asked: serde_json::Map<String, Value> = serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
            Ok(Value::Object(asked.keys().filter(|k| !k.starts_with("omit:")).map(|k| (k.clone(), json!(format!("中 {k}")))).collect()).to_string())
        };
        async move {
            match reply {
                Ok(content) => axum::Json(json!({"choices": [{"message": {"content": content}}]})).into_response(),
                Err((status, text)) => (axum::http::StatusCode::from_u16(status).unwrap(), text).into_response(),
            }
        }
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

/// The tags a tag request asks for, in id order: the JSON array on the user message's last line.
pub(super) fn asked_in(request: &Value) -> Vec<String> {
    let items: Vec<Value> = serde_json::from_str(request["messages"][1]["content"].as_str().unwrap().lines().last().unwrap()).unwrap();
    items.iter().map(|item| item["tag"].as_str().unwrap().to_owned()).collect()
}

/// The tags each tag request asked for, request by request, sorted.
pub(super) fn asked_tags(requests: &[Value]) -> Vec<Vec<String>> {
    let tag_requests = requests.iter().filter(|r| r["messages"][0]["content"] == crate::ai::TAG_SYSTEM);
    tag_requests
        .map(|r| {
            let mut tags = asked_in(r);
            tags.sort();
            tags
        })
        .collect()
}

/// A reply keyed by id with each asked tag's raw JSON answer from `answers` (none for a tag it lacks), the tag
/// copied into each answer object as the reply format asks.
pub(super) fn by_id(tags: &[String], answers: &[(&str, &str)]) -> String {
    let echoed = |tag: &str, raw: &str| match raw.strip_prefix('{') {
        Some(rest) => format!("{{\"tag\": {}, {rest}", json!(tag)),
        None => raw.to_owned(),
    };
    let entries: Vec<String> = tags
        .iter()
        .enumerate()
        .filter_map(|(i, tag)| answers.iter().find(|(t, _)| t == tag).map(|(_, raw)| format!("\"{}\": {}", crate::ai::tag_id(i), echoed(tag, raw))))
        .collect();
    format!("{{{}}}", entries.join(", "))
}

pub(super) async fn localize_job(pool: &PgPool, params: Value) -> String {
    crate::install_tls();
    let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('localize', 'running', now()) returning id").fetch_one(pool).await.unwrap();
    let booth = Booth::new().unwrap();
    let mut ctx = Ctx::new(pool, &booth, job);
    let summary = localize(&mut ctx, &params).await.unwrap();
    sqlx::query("update jobs set status = 'succeeded' where id = $1").bind(job).execute(pool).await.unwrap();
    summary
}

pub(super) type DictionaryRow = (String, Option<String>, Option<String>, Option<String>, Option<String>, Vec<String>, String);

/// The dictionary's rows in Rust's order of the tags (the database's collation orders Hangul differently).
pub(super) async fn dictionary(pool: &PgPool) -> Vec<DictionaryRow> {
    let mut rows: Vec<DictionaryRow> = sqlx::query_as("select tag, en, ja, ko, zh, keywords, source from booth_tag_translations").fetch_all(pool).await.unwrap();
    rows.sort();
    rows
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn localize_gives_missing_tags_a_row_and_keeps_every_row_there(pool: PgPool) {
    sqlx::raw_sql("update smart_categories set name_zh = 'x', group_zh = 'x'; update smart_tags set display_zh = 'x';").execute(&pool).await.unwrap();
    for (ids, status, tags) in [
        (1..=5, "ok", "{テクスチャ,スカート,マヌカ,マヌカ用,ポジ,ミミ,신라}"),
        (6..=6, "delisted", "{テクスチャ,スカート,マヌカ,マヌカ用,ポジ,ミミ,신라}"),
        (7..=10, "ok", "{レア}"),
        (11..=11, "delisted", "{レア}"),
        (12..=16, "ok", "{壊れ}"),
    ] {
        for id in ids {
            sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, likes, booth_tags) values ($1, $2, 'c', 'https://example.test', false, $3, now(), $1, $4::text[])")
                .bind(id as i64).bind(format!("Item {id}")).bind(status).bind(tags).execute(&pool).await.unwrap();
        }
    }
    sqlx::query("insert into booth_tag_translations (tag, en, ja, ko, zh, source) values ('スカート', 'skirt', 'スカート', '스커트', '裙子', 'import')").execute(&pool).await.unwrap();
    let answers = [
        ("テクスチャ", r#"{"en": "texture", "ja": "テクスチャ", "ko": "텍스처", "zh": "纹理"}"#),
        ("マヌカ用", r#"{"en": "for Manuka", "ja": "マヌカ用", "ko": "마누카용", "zh": "マヌカ用"}"#),
        ("신라", r#"{"en": "Shinra", "ja": "森羅", "ko": "森羅", "zh": "森羅"}"#),
        ("壊れ", r#"{"en": "broken", "ja": "壊れ", "ko": "고장", "zh": "损坏\n坏了"}"#),
        ("ポジ", r#"["positive", "ポジ", "포지", "正面"]"#),
        ("ミミ", r#"{"id": "t1", "tag": "ミミ", "id": "t1", "context": ["Item 5"]}"#),
    ];
    let requests = stand_in_provider(&pool, move |_, tags| Ok(by_id(tags, &answers))).await;

    let version = || async { sqlx::query_scalar::<_, i64>("select version from catalog_meta").fetch_one(&pool).await.unwrap() };
    let before = version().await;
    assert_eq!(localize_job(&pool, json!({"max_tags": 0})).await, "0 names translated to Chinese (0 unique strings); no new tags");
    assert!(requests.lock().unwrap().is_empty());
    assert_eq!(version().await, before);
    let summary = localize_job(&pool, json!({})).await;
    assert_eq!(summary, "0 names translated to Chinese (0 unique strings); 5 tags added to the dictionary (1 avatar names, 2 with a language left empty), 2 without a valid translation");
    assert_eq!(version().await, before + 1);
    let row = |tag: &str, v: [Option<&str>; 4], keywords: &[&str], source: &str| {
        let [en, ja, ko, zh] = v.map(|v| v.map(str::to_owned));
        (tag.to_owned(), en, ja, ko, zh, keywords.iter().map(|k| k.to_string()).collect(), source.to_owned())
    };
    let mut expected = vec![
        row("マヌカ", [Some("Manuka"), Some("マヌカ"), Some("マヌカ"), Some("マヌカ")], &[], "localize"),
        row("マヌカ用", [Some("for Manuka"), Some("マヌカ用"), None, Some("マヌカ用")], &[], "localize"),
        row("壊れ", [Some("broken"), Some("壊れ"), Some("고장"), None], &[], "localize"),
        row("スカート", [Some("skirt"), Some("スカート"), Some("스커트"), Some("裙子")], &[], "import"),
        row("テクスチャ", [Some("texture"), Some("テクスチャ"), Some("텍스처"), Some("纹理")], &["贴图"], "localize"),
        row("신라", [Some("Shinra"), Some("森羅"), Some("森羅"), Some("森羅")], &[], "localize"),
    ];
    expected.sort();
    assert_eq!(dictionary(&pool).await, expected);

    let sent = requests.lock().unwrap().clone();
    assert_eq!(asked_tags(&sent), [vec!["テクスチャ", "ポジ", "マヌカ用", "ミミ", "壊れ", "신라"]]);
    let format = &sent[0]["response_format"];
    assert_eq!((format["type"].as_str(), format["json_schema"]["strict"].as_bool()), (Some("json_schema"), Some(true)));
    assert_eq!(format["json_schema"]["schema"], crate::ai::tag_schema(6));
    let (names, items) = sent[0]["messages"][1]["content"].as_str().unwrap().split_once('\n').unwrap();
    assert_eq!(names, "Avatar names (Japanese / romaji): マヌカ / Manuka");
    let items: Vec<Value> = serde_json::from_str(items).unwrap();
    assert_eq!(items.iter().find(|i| i["tag"] == "テクスチャ").unwrap()["context"], json!(["Item 5", "Item 4", "Item 3"]));
    let system = sent[0]["messages"][0]["content"].as_str().unwrap();
    for rule in ["tag copies the item's tag", "never translate it or copy it", "never translated or transliterated", "Product, app, tool, brand and shop names stay as written", "白模", "虚拟形象 for an avatar", "纹理 for a texture"] {
        assert!(system.contains(rule), "{rule}: {system}");
    }

    let failures = || async { sqlx::query_as::<_, (String, i32)>("select tag, failures from booth_tag_failures order by tag").fetch_all(&pool).await.unwrap() };
    for run in 2..=3 {
        assert_eq!(failures().await, [("ポジ".to_owned(), run - 1), ("ミミ".to_owned(), run - 1)]);
        requests.lock().unwrap().clear();
        assert_eq!(localize_job(&pool, json!({})).await, "0 names translated to Chinese (0 unique strings); 0 tags added to the dictionary (0 avatar names, 0 with a language left empty), 2 without a valid translation");
        assert_eq!(asked_tags(&requests.lock().unwrap()), [vec!["ポジ", "ミミ"]]);
        assert_eq!(version().await, before + 1);
    }
    requests.lock().unwrap().clear();
    assert_eq!(localize_job(&pool, json!({})).await, "0 names translated to Chinese (0 unique strings); no new tags");
    assert!(requests.lock().unwrap().is_empty());
    assert_eq!(dictionary(&pool).await, expected);

    let recorded = || async { sqlx::query_scalar::<_, Value>("select value from app_settings where key = 'localize_state'").fetch_optional(&pool).await.unwrap() };
    assert_eq!(recorded().await, None);
    requests.lock().unwrap().clear();
    let forced = localize_job(&pool, json!({"force": true})).await;
    assert!(forced.ends_with(" unique strings)") && !forced.starts_with("0 "), "{forced}");
    assert!(asked_tags(&requests.lock().unwrap()).is_empty());
    assert_eq!(dictionary(&pool).await, expected);
    let untranslated: i64 = sqlx::query_scalar("select (select count(*) from smart_categories where name_zh not like '中 %') + (select count(*) from smart_tags where display_zh not like '中 %')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(untranslated, 0);
    assert_eq!(recorded().await, Some(json!({"prompt_version": boothdex_core::LOCALIZE_PROMPT_VERSION})));

    sqlx::raw_sql("delete from app_settings where key = 'localize_state'; update smart_tags set display = 'omit: ' || display where id = (select min(id) from smart_tags);").execute(&pool).await.unwrap();
    localize_job(&pool, json!({"force": true})).await;
    assert_eq!(recorded().await, None);
    sqlx::query("update smart_tags set display = substr(display, 7) where display like 'omit: %'").execute(&pool).await.unwrap();
    localize_job(&pool, json!({"force": true})).await;
    assert_eq!(recorded().await, Some(json!({"prompt_version": boothdex_core::LOCALIZE_PROMPT_VERSION})));
}
