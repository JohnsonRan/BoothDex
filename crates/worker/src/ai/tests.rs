use super::*;

#[tokio::test(start_paused = true)]
async fn exhausted_retries_return_provider_error_without_extra_sleep() {
    crate::install_tls();
    for status in [429, 503] {
        let (base_url, requests) = crate::retry_test_server(status.try_into().unwrap(), None).await;
        let ai = Ai {
            http: reqwest::Client::builder().no_proxy().pool_max_idle_per_host(0).build().unwrap(),
            base_url,
            model: "test".into(),
            api_key: "test".into(),
            reasoning_effort: "none",
        };
        let start = tokio::time::Instant::now();
        let error = ai.json("system", "user").await.unwrap_err();
        assert!(error.downcast_ref::<ProviderError>().is_some());
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 5);
        assert_eq!(start.elapsed(), Duration::from_secs(30));
    }
}

#[tokio::test]
async fn correction_resends_rejected_reply_and_error() {
    crate::install_tls();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Value::Null));
    let store = seen.clone();
    let app = axum::Router::new().fallback(move |axum::Json(body): axum::Json<Value>| {
        *store.lock().unwrap() = body;
        async { axum::Json(json!({"choices": [{"message": {"content": "{\"ok\": true}"}}]})) }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let ai = Ai { http: reqwest::Client::builder().no_proxy().build().unwrap(), base_url, model: "m".into(), api_key: "k".into(), reasoning_effort: "none" };

    let rejected = json!({"categories": ["animations-x"]});
    let reply = ai.json_correction("sys", "item", &rejected, "missing or invalid categories in AI reply: [\"animations-x\"]").await.unwrap();
    assert_eq!(reply, json!({"ok": true}));
    let msgs = seen.lock().unwrap()["messages"].clone();
    let roles: Vec<&str> = msgs.as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, ["system", "user", "assistant", "user"]);
    assert_eq!(msgs[2]["content"], rejected.to_string());
    let fix = msgs[3]["content"].as_str().unwrap();
    assert!(fix.contains("animations-x") && fix.contains("first word"), "{fix}");
}

#[test]
fn parses_fenced_json_and_escaped_quotes_and_rejects_reversed_braces() {
    assert_eq!(parse_json_reply("```json\n{\"a\": 1}\n```").unwrap(), json!({"a": 1}));
    assert!(parse_json_reply("sorry").is_err());
    assert!(parse_json_reply("} text {").is_err());
    let reply = r#"{"translations":{"ja":{"title":"SPS対応『最強快感巧具\"轟\" HM-25V Pecker』 \\ VRChat"}}}"#;
    assert_eq!(parse_json_reply(reply).unwrap()["translations"]["ja"]["title"], "SPS対応『最強快感巧具\"轟\" HM-25V Pecker』 \\ VRChat");
    assert!(parse_json_reply(r#"{"title":"最強快感巧具"轟""}"#).is_err());
}

#[test]
fn localize_uses_the_chinese_terms_of_vrchat_users_and_unity() {
    for term in ["虚拟形象 for an avatar", "3D模型 for a 3D model file", "纹理 for a texture", "法线贴图 for a normal map", "材质 for a material", "着色器 for a shader", "网格 for a mesh"] {
        assert!(LOCALIZE_SYSTEM.contains(term), "{term}");
    }
}

#[test]
fn enrichment_prompt_includes_classification_evidence() {
    let categories = "accessories--rings: Accessories / Rings\nprops--other: Props / Other";
    let item = EnrichItem {
        id: 1,
        title: "【VRChat想定】No Signal Ring【MA対応】".into(),
        booth_category: Some("3D小道具".into()),
        price_jpy: Some(500),
        booth_tags: vec!["VRChat".into(), "Modular Avatar".into()],
        variations: sqlx::types::Json(vec![Variation { name: "Full set".into(), price_jpy: json!(500) }]),
        bio: "An animated ring accessory for avatar fingers.".into(),
        translations: json!({}),
    };
    let prompt = enrich_prompt(&item, categories, &[]);
    for evidence in [
        categories,
        "Title: 【VRChat想定】No Signal Ring【MA対応】",
        "Booth category: 3D小道具",
        "Listed price: ¥500",
        "Tags: VRChat, Modular Avatar",
        "- Full set (¥500)",
        "Variations:\n- Full set (¥500)\nDescription:\nAn animated ring accessory for avatar fingers.",
    ] {
        assert!(prompt.contains(evidence), "missing evidence: {evidence}");
    }
    let manuka = AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() };
    let adolescent = AvatarName { ja: Some("Adolescent U".into()), romaji: "Adolescent U".into() };
    let unnamed = AvatarName { ja: None, romaji: "Rusk".into() };
    let prompt = enrich_prompt(&item, categories, &[&manuka, &adolescent, &unnamed]);
    assert!(prompt.contains("- Full set (¥500)\nAvatar names (Japanese / romaji): マヌカ / Manuka, Adolescent U, Rusk\nDescription:\n"), "{prompt}");
}

#[test]
fn a_provider_without_json_schema_is_told_apart_from_other_refusals() {
    assert!(refuses_format(400, "bad request"));
    assert!(refuses_format(422, r#"{"error": "unsupported response_format type json_schema"}"#));
    assert!(!refuses_format(401, "invalid api key") && !refuses_format(404, "model not found"));
}

#[test]
fn the_reply_schema_asks_for_every_id_with_the_tag_and_four_string_values() {
    let schema = tag_schema(2);
    assert_eq!(schema["required"], json!(["t1", "t2"]));
    assert_eq!(schema["additionalProperties"], json!(false));
    let tag = &schema["properties"]["t2"];
    assert_eq!((tag["required"].clone(), tag["additionalProperties"].clone()), (json!(["tag", "en", "ja", "ko", "zh"]), json!(false)));
    assert_eq!(tag["properties"]["zh"], json!({"type": "string"}));
    assert_eq!(tag["properties"]["tag"], json!({"type": "string"}));
}

#[test]
fn tag_prompt_names_the_avatars_and_keys_the_tags_by_id() {
    let tags = vec![("マヌカ用".to_owned(), vec!["a".repeat(130), "b".into(), "c".into(), "d".into()]), ("白モデル".to_owned(), vec![])];
    let manuka = AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() };
    let prompt = tag_prompt(&tags, &[&manuka]);
    let (head, body) = prompt.split_once('\n').unwrap();
    assert_eq!(head, "Avatar names (Japanese / romaji): マヌカ / Manuka");
    let items: Value = serde_json::from_str(body).unwrap();
    assert_eq!(items, json!([{"id": "t1", "tag": "マヌカ用", "context": ["a".repeat(120), "b", "c"]}, {"id": "t2", "tag": "白モデル", "context": []}]));
    assert!(tag_prompt(&tags, &[]).starts_with("[{"));
}

#[test]
fn validation_rejects_empty_or_missing_essentials() {
    let known = vec!["clothing--dresses".to_owned(), "hair--long".to_owned()];
    let full = json!({
        "categories": ["clothing--dresses"], "content_rating": "SFW", "features": [], "smart_price": 500,
        "translations": {"en": {"title": "Dress"}, "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}},
    });
    assert!(validate_enrichment(&json!({}), &known, &json!({})).is_err());
    assert!(validate_enrichment(&json!({"content_rating": "SFW", "categories": ["clothing--dresses"]}), &known, &json!({})).is_err());
    for drop_key in ["categories", "content_rating", "features", "smart_price"] {
        let mut bad = full.clone();
        bad.as_object_mut().unwrap().remove(drop_key);
        assert!(validate_enrichment(&bad, &known, &json!({})).is_err(), "{drop_key}");
    }
    let mut missing_ko = full.clone();
    missing_ko["translations"].as_object_mut().unwrap().remove("ko");
    assert!(validate_enrichment(&missing_ko, &known, &json!({})).is_err());
}

#[test]
fn category_with_dropped_leading_word_resolves() {
    let known = vec![
        "avatar-animations-physbones-gestures-and-gimmicks--dance-pose-and-emote-animations".to_owned(),
        "avatar-accessories--other".to_owned(),
        "avatar-props--other".to_owned(),
    ];
    let r = |s| resolve_category(s, &known);
    assert_eq!(r("animations-physbones-gestures-and-gimmicks--dance-pose-and-emote-animations"), Some(known[0].as_str()));
    assert_eq!(r("avatar-props--other"), Some(known[2].as_str()));
    assert_eq!(r("animations-physbones-gestures-and-gimmicks"), None);
    assert_eq!(r("dance-pose-and-emote-animations"), None);
    assert_eq!(r("--other"), None);
    assert_eq!(r("nimations-physbones-gestures-and-gimmicks--dance-pose-and-emote-animations"), None);
}

#[test]
fn validation_accepts_full_reply_null_smart_price_and_strict_iso_language() {
    let known = vec!["clothing--dresses".to_owned(), "hair--long".to_owned()];
    let mut v = json!({
        "categories": ["clothing--dresses", "made-up", "hair--long", "hair--long"],
        "features": ["liltoon"], "content_rating": "SFW", "quest_compatible": "Yes", "smart_price": 890,
        "translations": {
            "en": {"title": "Dress", "bio": "b", "variations": {"A": "String Val", "B": {"nested": "bad"}, "C": 123}},
            "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}
        },
        "original_language": "ja"
    });
    let e = validate_enrichment(&v, &known, &json!({})).unwrap();
    assert_eq!(e.categories, ["clothing--dresses", "hair--long"]);
    assert_eq!(e.features, ["liltoon"]);
    assert_eq!(e.content_rating.as_deref(), Some("SFW"));
    assert_eq!(e.quest.as_deref(), Some("Yes"));
    assert_eq!(e.smart_price, Some(890));
    assert_eq!(e.translations, json!({"en": {"title": "Dress"}, "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}, "original_language": "ja"}));

    v["smart_price"] = Value::Null;
    assert_eq!(validate_enrichment(&v, &known, &json!({})).unwrap().smart_price, None);

    // non-ASCII-lowercase or NUL-bearing original_language is ignored instead of reaching jsonb
    for bad in ["a\0", "é", "JA", "j1"] {
        v["original_language"] = json!(bad);
        let e = validate_enrichment(&v, &known, &json!({})).unwrap();
        assert!(e.translations.get("original_language").is_none(), "{bad:?}");
    }
}

#[test]
fn titles_replace_old_translations_and_chinese_is_optional() {
    let known = vec!["clothing--dresses".to_owned()];
    let reply = |zh: Option<Value>| {
        let mut t = json!({"en": {"title": "Dress"}, "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}});
        if let Some(zh) = zh {
            t["zh"] = zh;
        }
        json!({"categories": ["clothing--dresses"], "content_rating": "SFW", "features": [], "smart_price": null, "translations": t})
    };
    let existing = json!({
        "en": {"title": "Old", "bio": "Full English description", "variations": {"A": "Set A"}},
        "ja": {"title": "旧", "bio": "日本語の説明", "variations": {"A": "セットA"}},
        "original_language": "ja"
    });
    let e = validate_enrichment(&reply(None), &known, &existing).unwrap();
    assert_eq!(e.translations, json!({"en": {"title": "Dress"}, "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}, "original_language": "ja"}));

    let e = validate_enrichment(&reply(Some(json!({"title": "连衣裙", "bio": "说明"}))), &known, &json!({})).unwrap();
    assert_eq!(e.translations["zh"], json!({"title": "连衣裙"}));
    assert!(validate_enrichment(&reply(Some(Value::Null)), &known, &json!({})).unwrap().translations.get("zh").is_none());
    let old = json!({"zh": {"title": "旧", "bio": "", "variations": {}}});
    assert_eq!(validate_enrichment(&reply(Some(json!({"title": ""}))), &known, &old).unwrap().translations["zh"], json!({"title": "旧"}));
}

#[test]
fn request_sends_reasoning_effort_as_configured() {
    crate::install_tls();
    let mut c = boothdex_core::settings::resolve(&json!({ "base_url": "http://x/v1", "model": "m", "api_key": "k" }), |_| None);
    let ai = Ai::from_config(&c).unwrap().unwrap();
    assert_eq!(ai.body(json!([]), json!({}))["reasoning_effort"], "none");
    c.reasoning_effort = "high";
    assert_eq!(Ai::from_config(&c).unwrap().unwrap().body(json!([]), json!({}))["reasoning_effort"], "high");
}
