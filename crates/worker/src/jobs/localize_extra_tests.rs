use std::collections::HashMap;

use serde_json::{Value, json};
use sqlx::PgPool;

use super::Ctx;
use super::avatar_kit::AvatarKit;
use super::localize::localize;
use super::localize_tests::{asked_in, asked_tags, by_id, dictionary, localize_job, stand_in_provider};
use crate::booth::Booth;

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn localize_asks_for_plain_json_once_the_provider_refuses_json_schema(pool: PgPool) {
    sqlx::raw_sql("update smart_categories set name_zh = 'x', group_zh = 'x'; update smart_tags set display_zh = 'x';").execute(&pool).await.unwrap();
    // 41 tags on 5 listed items: two batches
    let tags: Vec<String> = (0..=40).map(|n| format!("タグ{n:02}")).collect();
    for id in 1..=5i64 {
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags) values ($1, 'Item', 'c', 'https://example.test', false, 'ok', now(), $2)")
            .bind(id)
            .bind(&tags)
            .execute(&pool)
            .await
            .unwrap();
    }
    let requests = stand_in_provider(&pool, |body, tags| {
        if body["response_format"]["type"] == "json_schema" {
            return Err((400, r#"{"error": {"message": "response_format type json_schema is not supported"}}"#.into()));
        }
        let answers: Vec<String> = tags.iter().map(|t| format!(r#"{{"en": "tag {0}", "ja": "{t}", "ko": "태그 {0}", "zh": "标签 {0}"}}"#, &t[t.len() - 2..])).collect();
        let answers: Vec<(&str, &str)> = tags.iter().map(String::as_str).zip(answers.iter().map(String::as_str)).collect();
        Ok(by_id(tags, &answers))
    })
    .await;
    let summary = localize_job(&pool, json!({})).await;
    assert_eq!(summary, "0 names translated to Chinese (0 unique strings); 41 tags added to the dictionary (0 avatar names, 0 with a language left empty), 0 without a valid translation");
    let sent = requests.lock().unwrap().clone();
    let asked: Vec<(&str, usize)> = sent.iter().map(|r| (r["response_format"]["type"].as_str().unwrap(), asked_in(r).len())).collect();
    assert_eq!(asked, [("json_schema", 40), ("json_object", 40), ("json_object", 1)]);
    let zh: Option<String> = sqlx::query_scalar("select zh from booth_tag_translations where tag = 'タグ40'").fetch_one(&pool).await.unwrap();
    assert_eq!(zh.as_deref(), Some("标签 40"));
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn localize_discards_an_answer_filed_under_another_tags_id(pool: PgPool) {
    sqlx::raw_sql("update smart_categories set name_zh = 'x', group_zh = 'x'; update smart_tags set display_zh = 'x';").execute(&pool).await.unwrap();
    for id in 1..=5i64 {
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags) values ($1, 'Item', 'c', 'https://example.test', false, 'ok', now(), '{スカート,ドレス,帽子}')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let requests = stand_in_provider(&pool, |_, tags| {
        let answer = |i: usize, tag: &str| json!({"tag": tag, "en": format!("tag {i}"), "ja": tag, "ko": format!("태그 {i}"), "zh": format!("标签 {i}")});
        let mut reply: serde_json::Map<String, Value> = tags.iter().enumerate().map(|(i, t)| (crate::ai::tag_id(i), answer(i, t))).collect();
        let (first, second) = (reply["t1"].clone(), reply["t2"].clone());
        (reply["t1"], reply["t2"]) = (second, first);
        Ok(Value::Object(reply).to_string())
    })
    .await;
    let summary = localize_job(&pool, json!({})).await;
    assert_eq!(summary, "0 names translated to Chinese (0 unique strings); 1 tags added to the dictionary (0 avatar names, 0 with a language left empty), 2 without a valid translation");
    let asked = asked_in(&requests.lock().unwrap()[0]);
    let rows: Vec<String> = sqlx::query_scalar("select tag from booth_tag_translations").fetch_all(&pool).await.unwrap();
    assert_eq!(rows, [asked[2].clone()]);
    let failures: Vec<(String, i32)> = sqlx::query_as("select tag, failures from booth_tag_failures order by tag").fetch_all(&pool).await.unwrap();
    let mut swapped = vec![(asked[0].clone(), 1), (asked[1].clone(), 1)];
    swapped.sort();
    assert_eq!(failures, swapped);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn localize_stops_when_the_provider_refuses_a_tag_request(pool: PgPool) {
    sqlx::raw_sql("update smart_categories set name_zh = 'x', group_zh = 'x'; update smart_tags set display_zh = 'x';").execute(&pool).await.unwrap();
    for id in 1..=5i64 {
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags) values ($1, 'Item', 'c', 'https://example.test', false, 'ok', now(), '{スカート}')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    stand_in_provider(&pool, |_, _| Err((401, "invalid api key".into()))).await;
    let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('localize', 'running', now()) returning id").fetch_one(&pool).await.unwrap();
    let booth = Booth::new().unwrap();
    let err = localize(&mut Ctx::new(&pool, &booth, job), &json!({})).await.unwrap_err();
    assert!(err.downcast_ref::<crate::ai::ProviderError>().is_some_and(|e| e.0.contains("401")), "{err:#}");
    let (rows, failures): (i64, i64) = sqlx::query_as("select (select count(*) from booth_tag_translations), (select count(*) from booth_tag_failures)").fetch_one(&pool).await.unwrap();
    assert_eq!((rows, failures), (0, 0));
}

#[test]
fn a_tag_holds_an_avatar_name_in_the_forms_it_writes_and_in_japanese_and_romaji() {
    let names = |romaji: &str, ja: &str, ko: &str| vec![romaji.to_owned(), ja.to_owned(), ko.to_owned()];
    let forms: HashMap<String, Vec<String>> = [("manuka".to_owned(), names("Manuka", "マヌカ", "마누카")), ("alue".to_owned(), names("Alué", "アルエ", "アルエ"))].into();
    let avatar = |ja: &str, romaji: &str| crate::ai::AvatarName { ja: Some(ja.into()), romaji: romaji.into() };
    let kit = AvatarKit {
        matcher: crate::avatars::Matcher::new(forms.clone()),
        names: [("manuka".to_owned(), avatar("マヌカ", "Manuka")), ("alue".to_owned(), avatar("アルエ", "Alué"))].into(),
        forms,
        romaji: Default::default(),
        japanese: Vec::new(),
    };
    let held = |tag: &str| kit.held_names(tag).into_iter().map(|n| (n.forms.join(" "), n.latin.join(" "))).collect::<Vec<_>>();
    let pair = |forms: &str, latin: &str| (forms.to_owned(), latin.to_owned());
    // the Korean name counts only where the tag writes it, so a Korean value transliterating マヌカ is caught
    assert_eq!(held("マヌカ用"), [pair("マヌカ manuka", "manuka")]);
    assert_eq!(held("마누카용"), [pair("마누카 マヌカ manuka", "manuka")]);
    // writing both Latin and Japanese never duplicates forms
    assert_eq!(held("Manuka マヌカ"), [pair("manuka マヌカ", "manuka")]);
    assert!(held("スカート").is_empty());
    assert_eq!(held("【アルエ対応】ドレス"), [pair("アルエ alué", "alué")]);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn localize_without_a_provider_is_skipped(pool: PgPool) {
    for key in ["AI_BASE_URL", "AI_API_KEY", "AI_MODEL"] {
        assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}");
    }
    assert_eq!(localize_job(&pool, json!({})).await, "skipped: AI provider not configured");
    let (enabled, minutes, later): (bool, i32, bool) =
        sqlx::query_as("select enabled, interval_minutes, next_run_at > now() + interval '23 hours' from job_schedules where kind = 'localize'").fetch_one(&pool).await.unwrap();
    assert_eq!((enabled, minutes, later), (true, 1440, true));
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn localize_asks_for_a_tag_that_is_an_alias_like_any_word(pool: PgPool) {
    sqlx::raw_sql("update smart_categories set name_zh = 'x', group_zh = 'x'; update smart_tags set display_zh = 'x'; update avatars set aliases = aliases || '{ぬぺぽ}' where slug = 'manuka';")
        .execute(&pool)
        .await
        .unwrap();
    for id in 1..=5i64 {
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags) values ($1, 'Item', 'c', 'https://example.test', false, 'ok', now(), '{ぬぺぽ}')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let answers = [("ぬぺぽ", r#"{"en": "nupepo", "ja": "ぬぺぽ", "ko": "누페포", "zh": "ぬぺぽ"}"#)];
    let requests = stand_in_provider(&pool, move |_, tags| Ok(by_id(tags, &answers))).await;
    localize_job(&pool, json!({})).await;
    assert_eq!(asked_tags(&requests.lock().unwrap()), [vec!["ぬぺぽ"]]);
    let row = dictionary(&pool).await.into_iter().find(|r| r.0 == "ぬぺぽ").unwrap();
    assert_eq!(row.1.as_deref(), Some("nupepo"));
}
