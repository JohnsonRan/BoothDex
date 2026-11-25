use super::labels::{AvatarNames, TagLabels, forms, respelling};
use super::tests::{catalogue, catalogue_with_words, labels, related, related_in, row};
use crate::text::fold;

#[test]
fn a_word_of_the_query_or_its_dictionary_translation_is_not_offered() {
    let c = catalogue(&[(20, "Black Long Hair", &["黒髪"], 5), (20, "Black Long Hair", &["ロング"], 5), (20, "Black Long Hair", &["ponytail"], 5)], |_, _| {});
    assert_eq!(labels(related(&c, &["黒髪", "ロング", "ponytail"], "q=黒髪%20ロング", &[], None)), ["ponytail"]);

    let c = catalogue(&[(20, "Dress", &["Dresses"], 5), (20, "Dress", &["テクスチャ"], 5), (20, "Dress", &["frills"], 5)], |_, _| {});
    let tags = ["Dresses", "テクスチャ", "frills"];
    let dictionary = || TagLabels::new(&[row("Dresses", [Some("dresses"), Some("ドレス"), Some("드레스"), Some("连衣裙")], &[]), row("テクスチャ", [Some("texture"), Some("テクスチャ"), Some("텍스처"), Some("纹理")], &["贴图"])], c.tags());
    assert_eq!(labels(related(&c, &tags, "q=ドレス", &[], None)), tags);
    assert_eq!(labels(related_in(&c, &tags, "q=ドレス", &[], None, dictionary(), None)), ["テクスチャ", "frills"]);
    assert_eq!(labels(related_in(&c, &tags, "q=贴图", &[], None, dictionary(), None)), ["Dresses", "frills"]);
}

#[test]
fn a_tag_the_dictionary_translates_as_a_word_of_the_query_is_not_offered() {
    let shinano = vec![("shinano".to_owned(), vec!["Shinano"])];
    let c = catalogue(&[(20, "Hair", &["ロング"], 5), (20, "Hair", &["ストレートヘア"], 5)], |_, d| d.avatars = vec!["shinano".into()]);
    let tags = ["ロング", "ストレートヘア"];
    let dictionary = || {
        TagLabels::new(
            &[
                row("ロング", [Some("Long"), Some("ロング"), Some("롱"), Some("长")], &[]),
                row("ストレートヘア", [Some("Straight Hair"), Some("ストレートヘア"), Some("스트레이트 헤어"), Some("直发")], &[]),
            ],
            c.tags(),
        )
    };
    assert_eq!(labels(related(&c, &tags, "q=Shinano%20long%20hair", &shinano, None)), tags);
    for (hl, label) in [(None, "ストレートヘア"), (Some("en"), "Straight Hair"), (Some("ja"), "ストレートヘア"), (Some("zh"), "直发")] {
        assert_eq!(labels(related_in(&c, &tags, "q=Shinano%20long%20hair", &shinano, None, dictionary(), hl)), [label], "{hl:?}");
    }
    let c = catalogue(&[(20, "Black Long Hair", &["long"], 5), (20, "Black Long Hair", &["ponytail"], 5)], |_, _| {});
    let dictionary = TagLabels::new(&[row("long", [Some("long"), Some("ロング"), None, None], &[])], c.tags());
    assert_eq!(labels(related_in(&c, &["long", "ponytail"], "q=黒髪%20ロング", &[], None, dictionary, Some("ja"))), ["ponytail"]);
}

#[test]
fn a_label_reading_in_the_visitors_language_as_the_querys_translation_is_not_offered() {
    let c = catalogue(&[(20, "ドレス", &["ドレス"], 5), (20, "One-piece", &["one-piece"], 5), (20, "Frills", &["frills"], 5)], |_, _| {});
    let tags = ["ドレス", "one-piece", "frills"];
    let dictionary = || {
        TagLabels::new(
            &[
                row("ドレス", [Some("dress"), Some("ドレス"), Some("드레스"), Some("连衣裙")], &[]),
                row("one-piece", [Some("one-piece dress"), Some("ワンピース"), Some("원피스"), Some("连衣裙")], &[]),
                row("frills", [Some("frills"), Some("フリル"), Some("프릴"), Some("荷叶边")], &[]),
            ],
            c.tags(),
        )
    };
    for (hl, offered) in [(None, &["one-piece", "frills"][..]), (Some("en"), &["one-piece dress", "frills"]), (Some("ja"), &["ワンピース", "フリル"]), (Some("zh"), &["荷叶边"])] {
        assert_eq!(labels(related_in(&c, &tags, "q=dress", &[], None, dictionary(), hl)), offered, "{hl:?}");
    }
    for query in ["q=gothic%20dress", "q=dresses"] {
        assert_eq!(labels(related_in(&c, &tags, query, &[], None, dictionary(), Some("zh"))), ["荷叶边"], "{query}");
    }
    let c = catalogue(&[(20, "Dress", &["Dresses"], 5), (20, "Gown", &["gown"], 5), (20, "Frills", &["frills"], 5)], |_, _| {});
    let dictionary = TagLabels::new(&[row("Dresses", [Some("dresses"), Some("ドレス"), None, None], &[]), row("gown", [Some("dress"), Some("ガウン"), None, None], &[])], c.tags());
    assert_eq!(labels(related_in(&c, &["Dresses", "gown", "frills"], "q=ドレス", &[], None, dictionary, Some("en"))), ["frills"]);
    let c = catalogue(&[(20, "テクスチャ", &["テクスチャ"], 5), (20, "Textures", &["Textures"], 5), (20, "Frills", &["frills"], 5)], |_, _| {});
    let dictionary = || TagLabels::new(&[row("テクスチャ", [Some("texture"), Some("テクスチャ"), None, Some("纹理")], &["贴图"])], c.tags());
    let tags = ["テクスチャ", "Textures", "frills"];
    assert_eq!(labels(related_in(&c, &tags, "q=贴图", &[], None, dictionary(), Some("en"))), ["frills"]);
    assert_eq!(labels(related_in(&c, &tags, "q=贴图", &[], None, dictionary(), None)), ["Textures", "frills"]);
}

#[test]
fn a_search_reads_in_the_visitors_language_and_searches_its_label_when_that_finds_items() {
    let words = crate::tag_words::TagWords::new([("frills".to_owned(), vec!["frills".to_owned(), "荷叶边".to_owned()])]);
    let groups: &[(usize, &str, &[&str], usize)] = &[(20, "Skirt", &["frills"], 5), (20, "Skirt", &["pleats"], 5), (20, "Skirt", &["pleated"], 5), (20, "Skirt", &["lace"], 5)];
    let c = catalogue_with_words(groups, |_, _| {}, &words);
    let tags = ["frills", "pleats", "pleated", "lace"];
    let dictionary = || {
        TagLabels::new(
            &[
                row("frills", [Some("frills"), Some("フリル"), Some("프릴"), Some("荷叶边")], &[]),
                row("pleats", [Some("pleats"), Some("プリーツ"), Some("플리츠"), Some("百褶")], &[]),
                row("pleated", [Some("pleated"), None, None, Some("百褶")], &[]),
            ],
            c.tags(),
        )
    };
    let pair = |label: &str, q: &str| (label.to_owned(), q.to_owned());
    assert_eq!(related_in(&c, &tags, "q=skirt", &[], None, dictionary(), Some("zh")), [pair("荷叶边", "荷叶边"), pair("百褶", "pleats"), pair("lace", "lace")]);
    assert_eq!(related_in(&c, &tags, "q=skirt", &[], None, dictionary(), Some("ko"))[2], pair("pleated", "pleated"));
    assert_eq!(related_in(&c, &tags, "q=skirt", &[], None, dictionary(), None)[0], pair("frills", "frills"));
    let shinano = vec![("shinano".to_owned(), vec!["Shinano"])];
    let c = catalogue_with_words(groups, |_, d| d.avatars = vec!["shinano".into()], &words);
    assert_eq!(related_in(&c, &tags, "q=Shinano%20skirt", &shinano, None, dictionary(), Some("zh"))[0], pair("荷叶边", "Shinano 荷叶边"));

    // one-character label is shown, but the tag is searched
    let words = crate::tag_words::TagWords::new([("ロング".to_owned(), vec!["长".to_owned()]), ("ストレートヘア".to_owned(), vec!["直发".to_owned()])]);
    let c = catalogue_with_words(&[(20, "Hair", &["ロング"], 5), (20, "Hair", &["ストレートヘア"], 5)], |_, d| d.avatars = vec!["shinano".into()], &words);
    let dictionary = TagLabels::new(&[row("ロング", [Some("Long"), Some("ロング"), Some("롱"), Some("长")], &[]), row("ストレートヘア", [Some("Straight Hair"), None, None, Some("直发")], &[])], c.tags());
    assert_eq!(related_in(&c, &["ロング", "ストレートヘア"], "q=Shinano%20黒髪", &shinano, None, dictionary, Some("zh")), [pair("长", "Shinano ロング"), pair("直发", "Shinano 直发")]);
}

#[test]
fn a_search_a_nearer_one_already_makes_is_left_out_and_the_next_takes_its_place() {
    let shinano = vec![("shinano".to_owned(), vec!["Shinano"])];
    let groups: &[(usize, &str, &[&str], usize)] = &[
        (20, "Straight", &["ストレートヘア"], 5),
        (20, "ウェーブヘア", &["ウェーブヘア"], 5),
        (20, "Wavy Hair", &["wavy hair"], 5),
        (20, "Ponytail", &["ponytail"], 5),
        (20, "Twin Tails", &["twin tails"], 5),
    ];
    let c = catalogue(groups, |_, d| d.avatars = vec!["shinano".into()]);
    let tags = ["ストレートヘア", "ウェーブヘア", "wavy hair", "ponytail", "twin tails"];
    let rows = |wavy: bool| {
        let mut rows = vec![
            row("ストレートヘア", [Some("Straight Hair"), Some("ストレートヘア"), None, Some("直发")], &[]),
            row("ウェーブヘア", [Some("wavy hair"), Some("ウェーブヘア"), Some("웨이브헤어"), Some("波浪发")], &[]),
        ];
        if wavy {
            rows.push(row("wavy hair", [Some("wavy hair"), Some("ウェーブヘア"), Some("웨이브 헤어"), Some("波浪发")], &[]));
        }
        rows
    };
    let query = "q=Shinano%20long%20hair";
    assert_eq!(labels(related(&c, &tags, query, &shinano, None)), ["ストレートヘア", "ウェーブヘア", "wavy hair", "ponytail"]);
    for wavy in [true, false] {
        for (hl, first) in [(None, ["ストレートヘア", "ウェーブヘア"]), (Some("en"), ["Straight Hair", "wavy hair"]), (Some("ja"), ["ストレートヘア", "ウェーブヘア"]), (Some("zh"), ["直发", "波浪发"])] {
            let dictionary = TagLabels::new(&rows(wavy), c.tags());
            assert_eq!(labels(related_in(&c, &tags, query, &shinano, None, dictionary, hl)), [first[0], first[1], "ponytail", "twin tails"], "{hl:?} {wavy}");
        }
    }
    let c = catalogue(&[(20, "Skirt", &["百褶"], 5), (20, "Skirt", &["pleats"], 5)], |_, _| {});
    let dictionary = TagLabels::new(&[row("百褶", [Some("pleated"), None, None, Some("百褶裙")], &[]), row("pleats", [Some("pleats"), None, None, Some("百褶")], &[])], c.tags());
    assert_eq!(related_in(&c, &["百褶", "pleats"], "q=skirt", &[], None, dictionary, Some("zh")), [("百褶裙".to_owned(), "百褶".to_owned())]);
    // duplicates skipped before counting toward VALIDATE
    let names: Vec<[String; 1]> = (0..10).map(|i| [format!("adult {i}")]).collect();
    let adult: Vec<[&str; 1]> = names.iter().map(|[n]| [n.as_str()]).collect();
    let mut groups: Vec<(usize, &str, &[&str], usize)> = vec![(20, "ウェーブヘア", &["ウェーブヘア"], 5), (20, "Wavy Hair", &["wavy hair"], 5)];
    groups.extend(adult.iter().map(|t| (20, "Hair", &t[..], 5)));
    groups.push((20, "Twin Tails", &["twin tails"], 5));
    let c = catalogue(&groups, |id, d| d.is_adult = (41..=240).contains(&id));
    let order: Vec<&str> = ["ウェーブヘア", "wavy hair"].into_iter().chain(adult.iter().map(|[t]| *t)).chain(["twin tails"]).collect();
    assert_eq!(labels(related_in(&c, &order, "q=long%20hair", &[], None, TagLabels::new(&rows(false), c.tags()), None)), ["ウェーブヘア", "twin tails"]);
}

#[test]
fn folded_tag_variants_partial_rows_and_avatar_colliding_labels_are_handled() {
    // case/width variant in booth_tag_translations resolves to canonical tag, preferring exact match when both exist
    let c = catalogue(&[(20, "Skirt", &["Pleats"], 5), (5, "Skirt", &["rare"], 5)], |_, _| {});
    let labels_folded = TagLabels::new(&[row("pleats", [Some("pleats"), None, None, Some("百褶")], &[]), row("rare", [Some("rare"), None, None, Some("稀有")], &[])], c.tags());
    assert_eq!((labels_folded.label("Pleats", 3), labels_folded.label("rare", 3)), (Some("百褶"), None));
    let labels_both = TagLabels::new(&[row("pleats", [None, None, None, Some("旧")], &[]), row("Pleats", [None, None, None, Some("新")], &[])], c.tags());
    assert_eq!(labels_both.label("Pleats", 3), Some("新"));

    // r.tag itself is indexed in spelt even when its own language value is missing (Finding 15)
    let c = catalogue(&[(20, "ドレス", &["ドレス"], 5), (20, "ドレス", &["gown"], 5), (20, "ドレス", &["frills"], 5)], |_, _| {});
    let partial = TagLabels::new(&[row("ドレス", [Some("dress"), None, None, None], &[]), row("gown", [Some("dress"), None, None, None], &[])], c.tags());
    assert_eq!(labels(related_in(&c, &["ドレス", "gown", "frills"], "q=ドレス", &[], None, partial, Some("en"))), ["frills"]);

    // translated label matching an avatar name ("百合" -> "Yuri") is excluded in English so clicking it won't filter by avatar
    let c = catalogue(&[(20, "Flower", &["百合"], 5), (20, "Flower", &["frills"], 5)], |_, _| {});
    let yuri = TagLabels::new(&[row("百合", [Some("Yuri"), Some("百合"), None, Some("百合")], &[])], c.tags());
    assert_eq!(labels(related_in(&c, &["百合", "frills"], "q=flower", &[], None, yuri, Some("en"))), ["frills"]);
}

#[test]
fn plurals_respellings_and_avatar_names_with_punctuation_behave_accurately() {
    let groups: &[(usize, &str, &[&str], usize)] = &[
        (20, "Eye Set", &["eye texture set"], 5),
        (20, "Eye Iris", &["eye texture"], 5),
        (20, "Eye Pupil", &["Eye Textures"], 5),
        (20, "Eye Highlight", &["Eyes"], 5),
        (20, "Eye Lash", &["pupil"], 5),
    ];
    let c = catalogue(groups, |_, _| {});
    assert_eq!(labels(related(&c, &["eye texture set", "eye texture", "Eye Textures", "Eyes", "pupil"], "q=eye", &[], None)), ["eye texture set", "eye texture", "pupil"]);
    let c = catalogue(&[(20, "Hoodie", &["Hoodies"], 5), (20, "Hoodie", &["zip"], 5)], |_, _| {});
    let dictionary = TagLabels::new(&[row("Hoodies", [Some("hoodies"), Some("パーカー"), Some("후드티"), Some("连帽衫")], &[])], c.tags());
    assert_eq!(labels(related_in(&c, &["Hoodies", "zip"], "q=hoodie", &[], None, dictionary, Some("ja"))), ["zip"]);

    let plurals = [("skirt", "Skirts"), ("gothic girl", "gothic girls"), ("dress", "Dresses"), ("eye", "Eyes"), ("eye texture", "Eye Textures"), ("hoodie", "Hoodies"), ("shoe", "shoes")];
    for (a, b) in plurals.into_iter().chain([("short hair", "ShortHair"), ("ドレス", "ﾄﾞﾚｽ")]) {
        assert!(respelling(a, b) && respelling(b, a), "{a} / {b}");
    }
    // symbols-only, short words, digits and CJK+'s' do not falsely collide through empty or stripped stems
    for (a, b) in [("スカート", "ミニスカート"), ("skirt", "long skirt"), ("hair", "chair"), ("♡", "★"), ("♡", "s"), ("s", "es"), ("a", "as"), ("10", "10s"), ("猫", "猫s")] {
        assert!(!respelling(a, b), "{a} / {b}");
    }
    assert_eq!(forms("").count(), 0);
    assert_eq!(forms("s").collect::<Vec<_>>(), ["s"]);

    let names = AvatarNames::new(["Kikyo", "桔梗", "io", "猫", "Rusk Dragon", "Aoi-chan", "Noshime:Re"]);
    for tag in ["kikyo", "kikyo (桔梗)", "kikyoドレス", "桔梗対応", "io", "猫", "rusk dragon", "rusk dragon dress", "aoi-chan衣装", "noshime:re用", "kikyo-dress"] {
        assert!(names.named_in(&fold(tag)), "{tag}");
    }
    for tag in ["io hair", "猫耳", "kikyodress", "rusk", "arusk dragon", "rusk dragonx", "aoi-chanx"] {
        assert!(!names.named_in(&fold(tag)), "{tag}");
    }
}
