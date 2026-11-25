use super::*;

/// Which of a tag's values `tag_values` keeps: `Some` for a kept value, `None` for one left out. The avatars known
/// by a Japanese name are マヌカ and 森羅.
fn kept(answer: &Value, tag: &str, titles: &[&str], names: &[&[&str]]) -> [Option<String>; 4] {
    let titles: Vec<String> = titles.iter().map(|t| t.to_string()).collect();
    let held = |forms: &&[&str]| {
        let forms: Vec<String> = forms.iter().map(|f| f.to_string()).collect();
        HeldName { latin: forms.iter().filter(|f| latin_letters(f)).cloned().collect(), forms }
    };
    let names: Vec<HeldName> = names.iter().map(held).collect();
    let avatars = [AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() }, AvatarName { ja: Some("森羅".into()), romaji: "Shinra".into() }];
    tag_values(answer, tag, &titles, &names, &AvatarName::japanese_names(&avatars)).map(Result::ok)
}

#[test]
fn each_tag_value_is_checked_on_its_own() {
    let skirt = |en: &str, ja: &str, ko: &str, zh: &str| json!({"en": en, "ja": ja, "ko": ko, "zh": zh});
    let all = |v: [&str; 4]| v.map(|s| Some(s.to_owned()));
    assert_eq!(kept(&skirt(" skirt ", "スカート", "스커트", "裙子"), "スカート", &[], &[]), all(["skirt", "スカート", "스커트", "裙子"]));
    // a second line, a control character, an overlong or empty value: only that language is left out
    for bad in ["裙子\n短裙", "裙\0子", "裙\t子", &"裙".repeat(61), " "] {
        assert_eq!(kept(&skirt("skirt", "スカート", "스커트", bad), "スカート", &[], &[])[3], None, "{bad:?}");
    }
    let overlong = kept(&skirt("skirt", "スカート", "스커트", &"裙".repeat(61)), "スカート", &[], &[]);
    assert!(overlong[..3].iter().all(Option::is_some) && overlong[3].is_none());
    // a script foreign to the language that the tag is not written in
    assert_eq!(kept(&skirt("スカート", "스커트", "裙子", "スカート"), "skirt", &[], &[]), [None, None, None, None]);
    assert_eq!(kept(&skirt("skirt", "スカート", "스커트", "스커트"), "Skirt", &[], &[])[3], None);
    // English never keeps kana or Han copied from a Japanese tag
    assert_eq!(kept(&skirt("スカート", "スカート", "스커트", "裙子"), "スカート", &[], &[])[0], None);
    let manuka: &[&str] = &["マヌカ", "manuka"];
    assert_eq!(kept(&skirt("for Manuka マヌカ", "マヌカ用", "マヌカ용", "マヌカ用"), "マヌカ用", &[], &[manuka])[0], None);
    // a script the tag is written in may stay in non-English values: the avatar name in マヌカ用
    let manuka_for = skirt("for Manuka", "マヌカ用", "マヌカ용", "マヌカ用");
    assert_eq!(kept(&manuka_for, "マヌカ用", &[], &[manuka]), all(["for Manuka", "マヌカ用", "マヌカ용", "マヌカ用"]));
    // middle dot U+30FB and double hyphen U+30A0 are punctuation, not kana
    let dotted = skirt("dress·ribbon", "ドレス・リボン", "드레스・리본", "连衣裙・蝴蝶结");
    assert_eq!(kept(&dotted, "dress", &[], &[]), all(["dress·ribbon", "ドレス・リボン", "드레스・리본", "连衣裙・蝴蝶结"]));
    // an avatar's Japanese name may stay in the Japanese, Korean and Chinese values of a tag written in other
    // letters (신라 is 森羅); English keeps the romaji, and other kana still count against Korean
    let shinra = skirt("Shinra", "森羅", "森羅", "森羅");
    assert_eq!(kept(&shinra, "신라", &[], &[]), all(["Shinra", "森羅", "森羅", "森羅"]));
    assert_eq!(kept(&skirt("森羅", "森羅", "マヌカ 텍스처", "マヌカ纹理"), "Manukatexture", &[], &[])[..], [None, Some("森羅".into()), Some("マヌカ 텍스처".into()), Some("マヌカ纹理".into())]);
    assert_eq!(kept(&skirt("Manuka eye", "マヌカの目", "マヌカの 눈", "マヌカの眼睛"), "Manukaeye", &[], &[])[2..], [None, None]);
    // a name transliterated or translated is left out in that language only; English may give the romaji
    let transliterated = skirt("for Manuka", "マヌカ用", "마누카용", "麦奴卡用");
    assert_eq!(kept(&transliterated, "マヌカ用", &[], &[manuka]), [Some("for Manuka".into()), Some("マヌカ用".into()), None, None]);
    assert_eq!(kept(&skirt("for the avatar", "マヌカ用", "マヌカ용", "マヌカ用"), "マヌカ用", &[], &[manuka])[0], None);
    assert_eq!(kept(&skirt("マヌカ用", "マヌカ用", "マヌカ용", "マヌカ用"), "マヌカ用", &[], &[manuka])[0], None);
    assert_eq!(kept(&skirt("for Tien", "ティエン用", "티엔용", "ティエン用"), "ティエン用", &[], &[&["ティエン"]])[0], Some("for Tien".into()));
    // an example title copied, whole or as a long part, unless it is the tag itself
    let titles = ["【無料】ふわふわロングスカート for Manuka", "スカート"];
    let copied = skirt("skirt", "ふわふわロングスカート for Manuka", "스커트", "【無料】ふわふわロングスカート for Manuka");
    assert_eq!(kept(&copied, "スカート", &titles, &[]), [Some("skirt".into()), None, Some("스커트".into()), None]);
    assert_eq!(kept(&skirt("skirt", "スカート", "스커트", "裙子"), "スカート", &titles, &[])[1], Some("スカート".into()));
    // malformed shapes under the tag's id
    assert_eq!(kept(&json!(["skirt", "スカート", "스커트", "裙子"]), "スカート", &[], &[]), [None, None, None, None]);
    let echoed: Value = serde_json::from_str(r#"{"id": "t1", "tag": "スカート", "id": "t1", "context": ["スカート"]}"#).unwrap();
    assert_eq!(kept(&echoed, "スカート", &[], &[]), [None, None, None, None]);
    assert_eq!(kept(&Value::Null, "スカート", &[], &[]), [None, None, None, None]);
}

#[test]
fn each_translated_title_keeps_the_names_adds_no_number_and_uses_its_script() {
    let japanese = AvatarName::japanese_names(&[
        AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() },
        AvatarName { ja: Some("しなの".into()), romaji: "Shinano".into() },
    ]);
    let manuka = HeldName { forms: vec!["マヌカ".into(), "manuka".into()], latin: vec!["manuka".into()] };
    let check = TitleCheck { original: "【マヌカ対応】フリルドレス 2着セット", names: vec![manuka], japanese: &japanese };
    let problem = |lang: &str, title: &str| check.problem(lang, title, Some("ja"));
    for (lang, title) in [
        ("en", "[For Manuka] Frill Dress, Set of 2"),
        ("ko", "【マヌカ 대응】프릴 드레스 2벌 세트"),
        ("zh", "【マヌカ対応】荷叶边连衣裙 两件套"),
        ("ja", "【マヌカ対応】フリルドレス 2着セット"),
    ] {
        assert_eq!(problem(lang, title), None, "{lang} {title}");
    }
    // Katakana middle dot U+30FB and double hyphen U+30A0 between avatar names or words are punctuation, not kana
    let pair_check = TitleCheck {
        original: "【マヌカ・しなの対応】フリルドレス",
        names: vec![
            HeldName { forms: vec!["マヌカ".into(), "manuka".into()], latin: vec!["manuka".into()] },
            HeldName { forms: vec!["しなの".into(), "shinano".into()], latin: vec!["shinano".into()] },
        ],
        japanese: &japanese,
    };
    assert_eq!(pair_check.problem("zh", "【マヌカ・しなの対応】荷叶边连衣裙", Some("ja")), None);
    assert_eq!(pair_check.problem("ko", "【マヌカ・しなの 대응】프릴 드레스", Some("ja")), None);
    assert_eq!(pair_check.problem("en", "[For Manuka・Shinano] Frill゠Dress", Some("ja")), None);
    // a NUL, which the database refuses
    assert_eq!(problem("en", "[For Manuka] Frill\0 Dress, Set of 2"), Some("holds a NUL character".into()));
    // English keeps the romaji; the others keep the name in any of its forms
    assert_eq!(problem("en", "[For マヌカ] Frill Dress 2"), Some("drops the avatar name manuka".into()));
    assert_eq!(problem("zh", "【适配麦奴卡】荷叶边连衣裙 2"), Some("drops the avatar name マヌカ".into()));
    assert_eq!(problem("zh", "【Manuka】荷叶边连衣裙 2"), None);
    // a name with no form in Latin letters gives an English title nothing to keep
    let tien = TitleCheck { original: "【ティエン対応】ドレス", names: vec![HeldName { forms: vec!["ティエン".into()], latin: vec![] }], japanese: &japanese };
    assert_eq!(tien.problem("en", "[For Tien] Dress", Some("ja")), None);
    assert_eq!(tien.problem("zh", "【适配蒂恩】连衣裙", Some("ja")), Some("drops the avatar name ティエン".into()));
    // digits that belong to an avatar's romaji (e.g. さん↑ぷらす→うの↑ / 3+1) are allowed in translated titles
    let three_plus_one = TitleCheck {
        original: "さん↑ぷらす→うの↑対応ドレス",
        names: vec![HeldName { forms: vec!["さん↑ぷらす→うの↑".into(), "3+1".into()], latin: vec!["3+1".into()] }],
        japanese: &japanese,
    };
    assert_eq!(three_plus_one.problem("en", "[For 3+1] Dress", Some("ja")), None);
    assert!(three_plus_one.problem("en", "[For 3+1] Dress 2", Some("ja")).is_some_and(|p| p.starts_with("has the number 2")));
    // a number the original lacks, in any digits; a Chinese numeral the original writes counts as its number
    for title in ["[For Manuka] Frill Dress, Set of 3", "[For Manuka] Frill Dress ３", "[For Manuka] Frill Dress 𝟑", "[For Manuka] 1,000 Frill Dresses"] {
        assert!(problem("en", title).is_some_and(|p| p.starts_with("has the number")), "{title}");
    }
    let three = TitleCheck { original: "三点セット 1,000円", names: vec![], japanese: &japanese };
    assert_eq!(three.problem("en", "3-piece set, 1000 yen", Some("ja")), None);
    assert_eq!(three.problem("en", "3-piece set, 01000 yen", Some("ja")), None);
    assert_eq!(TitleCheck { original: "ワンクリック設定", names: vec![], japanese: &japanese }.problem("zh", "一键设置", Some("ja")), None);
    // a script foreign to the language; an avatar's Japanese name is not, but in English
    assert_eq!(problem("zh", "【マヌカ対応】フリル连衣裙 2"), Some("holds kana, which a zh title does not use".into()));
    assert_eq!(problem("ko", "【マヌカ 대응】프릴 ドレス 2"), Some("holds kana, which a ko title does not use".into()));
    assert_eq!(problem("ko", "【マヌカ対応】프릴 드레스 2"), Some("holds Han characters, which a ko title does not use".into()));
    assert_eq!(problem("ja", "【マヌカ対応】フリルドレス 2벌"), Some("holds Hangul, which a ja title does not use".into()));
    assert_eq!(problem("en", "[For Manuka] マヌカ Frill Dress 2"), Some("holds kana, which a en title does not use".into()));
    // the listing's own language copies the original as it is; another language may not
    let chinese = TitleCheck { original: "猫の连衣裙", names: vec![], japanese: &japanese };
    assert_eq!(chinese.problem("zh", "猫の连衣裙", Some("zh")), None);
    assert!(chinese.problem("zh", "猫の连衣裙", Some("ja")).is_some());
    assert!(chinese.problem("ko", "猫の连衣裙", Some("zh")).is_some());
}

#[test]
fn a_failing_title_gives_way_to_an_earlier_one_that_passes_or_to_none() {
    let check = TitleCheck { original: "フリルドレス", names: vec![], japanese: &[] };
    let e = |translations: Value| Enrichment { categories: vec![], features: vec![], content_rating: None, quest: None, smart_price: None, translations };
    let reply = json!({"en": {"title": "Frill Dress 2"}, "ja": {"title": "フリルドレス"}, "ko": {"title": "프릴 ドレス"}, "zh": {"title": "荷叶边连衣裙"}, "original_language": "ja"});
    let existing = json!({"en": {"title": "Frill Dress"}, "ko": {"title": "프릴 드레스 3"}});
    assert_eq!(
        title_problems_message(&check.problems(&reply)),
        "translations.en.title has the number 2, which the original title lacks; translations.ko.title holds kana, which a ko title does not use"
    );
    let kept = check.keep_passing(e(reply), &existing).translations;
    assert_eq!(kept, json!({"en": {"title": "Frill Dress"}, "ja": {"title": "フリルドレス"}, "zh": {"title": "荷叶边连衣裙"}, "original_language": "ja"}));
}

#[test]
fn japanese_avatar_names_answers_tag_and_chinese_texture_keywords() {
    let name = |ja: Option<&str>| AvatarName { ja: ja.map(str::to_owned), romaji: "x".into() };
    let avatars = [name(Some("森羅")), name(Some("ルーナリット")), name(Some("Adolescent U")), name(Some("萌")), name(None), name(Some("ﾏﾇｶ")), name(Some("マヌカ"))];
    assert_eq!(AvatarName::japanese_names(&avatars), ["ルーナリット", "マヌカ", "森羅"]);

    assert!(answers_tag(&json!({"tag": " ｽｶｰﾄ ", "en": "skirt"}), "スカート"));
    assert!(answers_tag(&json!({"tag": "SKIRT"}), "skirt"));
    for wrong in [json!({"tag": "ドレス"}), json!({"en": "skirt"}), json!({"tag": 1}), json!(["skirt"]), Value::Null] {
        assert!(!answers_tag(&wrong, "スカート"), "{wrong}");
    }

    assert_eq!(tag_keywords("眼睛纹理"), ["眼睛贴图"]);
    assert_eq!(tag_keywords("纹理"), ["贴图"]);
    assert!(tag_keywords("法线贴图").is_empty() && tag_keywords("裙子").is_empty());
}
