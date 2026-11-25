//! Item -> avatar links by exact name matching against the avatar dictionary.
//! Measured on 10,089 archived items against VRCatalogue's own links (title + tags + variation names):
//! precision 0.87, recall 0.88. Descriptions are left out: they mention many avatars in passing and cut
//! precision to 0.74.

use anyhow::Result;
use sqlx::PgPool;

/// Every name the matcher reads, per avatar: the names in each language and the aliases.
pub async fn names(pool: &PgPool) -> Result<Vec<(String, Vec<String>)>> {
    Ok(sqlx::query_as("select slug, array_remove(array[name_en, name_ja, name_ko], null) || aliases from avatars order by slug").fetch_all(pool).await?)
}

/// Text the matcher searches: title, tags and variation names.
pub fn match_text<'a>(title: &str, tags: &[String], variation_names: impl IntoIterator<Item = &'a str>) -> String {
    format!("{} {} {}", title, tags.join(" "), variation_names.into_iter().collect::<Vec<_>>().join(" "))
}

pub struct Matcher {
    keys: Vec<(String, Vec<Name>)>,
}

enum Name {
    /// ASCII names match on word boundaries ("Lime" must not match "Limelight")
    Word(String),
    /// CJK names match as substrings
    Sub(String),
}

/// NFKC and lowercase: the form in which names and texts are compared.
pub fn norm(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkc().collect::<String>().to_lowercase()
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{4e00}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}')
}

impl Matcher {
    /// `avatars`: (slug, [names in any language])
    pub fn new(avatars: impl IntoIterator<Item = (String, Vec<String>)>) -> Self {
        let keys = avatars
            .into_iter()
            .map(|(slug, names)| {
                let mut out: Vec<Name> = Vec::new();
                for n in names.iter().map(|n| norm(n)) {
                    let cjk = n.chars().any(is_cjk);
                    // shorter names are too ambiguous as plain substrings
                    if n.chars().count() < if cjk { 2 } else { 4 } {
                        continue;
                    }
                    let name = if !cjk && n.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ') { Name::Word(n) } else { Name::Sub(n) };
                    if !out.iter().any(|o| matches!((o, &name), (Name::Word(a), Name::Word(b)) | (Name::Sub(a), Name::Sub(b)) if a == b)) {
                        out.push(name);
                    }
                }
                (slug, out)
            })
            .filter(|(_, n)| !n.is_empty())
            .collect();
        Self { keys }
    }

    pub fn find(&self, text: &str) -> Vec<String> {
        let t = norm(text);
        self.keys
            .iter()
            .filter(|(_, names)| {
                names.iter().any(|n| match n {
                    Name::Sub(s) => t.contains(s.as_str()),
                    Name::Word(w) => t.match_indices(w.as_str()).any(|(i, _)| {
                        let before = t[..i].chars().next_back();
                        let after = t[i + w.len()..].chars().next();
                        !before.is_some_and(|c| c.is_ascii_alphanumeric()) && !after.is_some_and(|c| c.is_ascii_alphanumeric())
                    }),
                })
            })
            .map(|(slug, _)| slug.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_names_in_any_language() {
        let m = Matcher::new([
            ("manuka".to_owned(), vec!["Manuka".to_owned(), "マヌカ".to_owned()]),
            ("lime".to_owned(), vec!["Lime".to_owned(), "ライム".to_owned()]),
            ("moe".to_owned(), vec!["Moe".to_owned(), "萌".to_owned()]),
        ]);
        assert_eq!(m.find("【マヌカ対応】ドレス"), ["manuka"]);
        assert_eq!(m.find("For MANUKA and Lime"), ["manuka", "lime"]);
        // word boundary for ASCII names; too-short names ("Moe", "萌") are never matched
        assert!(m.find("Limelight dress, moe style, 萌える").is_empty());
        assert_eq!(m.find("ライム用の髪"), ["lime"]);
        // full-width text is normalised
        assert_eq!(m.find("ＭＡＮＵＫＡ"), ["manuka"]);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn every_avatar_has_a_body_listing_and_names_of_its_own(pool: PgPool) {
        let incomplete: Vec<String> = sqlx::query_scalar("select slug from avatars where avatar_item_id is null or name_ko is null").fetch_all(&pool).await.unwrap();
        assert!(incomplete.is_empty(), "{incomplete:?}");
        // every name_en must be Latin letters so localize and enrich never write kana or Han as English romaji
        let romaji: Vec<(String, String)> = sqlx::query_as("select slug, name_en from avatars").fetch_all(&pool).await.unwrap();
        for (slug, name_en) in romaji {
            assert!(crate::ai::latin_letters(&name_en), "{slug}: non-Latin name_en {name_en:?}");
        }
        // an alias the matcher skips (too short) would be stored and never matched
        let aliases: Vec<(String, String)> = sqlx::query_as("select slug, unnest(aliases) from avatars").fetch_all(&pool).await.unwrap();
        assert!(!aliases.is_empty());
        for (slug, alias) in aliases {
            assert!(!Matcher::new([(slug.clone(), vec![alias.clone()])]).keys.is_empty(), "{slug}: {alias}");
        }
        // a name the matcher reads for two avatars would link both
        let mut owner: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for (slug, keys) in Matcher::new(names(&pool).await.unwrap()).keys {
            for key in keys {
                let (Name::Word(k) | Name::Sub(k)) = key;
                if let Some(other) = owner.insert(k.clone(), slug.clone()) {
                    panic!("{k} names {other} and {slug}");
                }
            }
        }
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn migration_0032_marks_body_listings_and_runs_again(pool: PgPool) {
        const SQL: &str = include_str!("../../core/migrations/0032_avatar_bodies.sql");
        // body listings of a new avatar (hakka), of a starter avatar without one until 0032 (miro, which VRCatalogue
        // marked supported) and of a starter avatar marked already (manuka); 1 is no body
        let manuka: i64 = sqlx::query_scalar("select avatar_item_id from avatars where slug = 'manuka'").fetch_one(&pool).await.unwrap();
        sqlx::query(
            "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, avatar_slug, supported, gender) values
               (2215270, 'hakka', 's', 'u', false, 'ok', now(), null, false, null),
               (7413817, 'miro', 's', 'u', false, 'ok', now(), null, true, 'male'),
               ($1, 'manuka', 's', 'u', false, 'ok', now(), 'manuka', true, 'female'),
               (1, 'outfit', 's', 'u', false, 'ok', now(), null, false, null)",
        )
        .bind(manuka)
        .execute(&pool)
        .await
        .unwrap();
        let state = || async {
            let items: Vec<(i64, Option<String>, bool, Option<String>)> =
                sqlx::query_as("select id, avatar_slug, supported, gender from items order by id").fetch_all(&pool).await.unwrap();
            let avatars: (i64, i64) = sqlx::query_as("select count(*), count(*) filter (where aliases <> '{}') from avatars").fetch_one(&pool).await.unwrap();
            (items, avatars)
        };
        sqlx::raw_sql(SQL).execute(&pool).await.unwrap();
        let after = state().await;
        let mut expected = vec![
            (1, None, false, None),
            (2215270, Some("hakka".to_owned()), true, Some("female".to_owned())),
            (7413817, Some("miro".to_owned()), true, Some("male".to_owned())),
            (manuka, Some("manuka".to_owned()), true, Some("female".to_owned())),
        ];
        expected.sort();
        assert_eq!(after.0, expected);
        assert_eq!(after.1, (598, 39));
        sqlx::raw_sql(SQL).execute(&pool).await.unwrap();
        assert_eq!(state().await, after);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn migration_0033_swaps_backwards_names_idempotently(pool: PgPool) {
        const SQL: &str = include_str!("../../core/migrations/0033_swap_avatar_names.sql");
        let rows = || async {
            sqlx::query_as::<_, (String, String, Option<String>)>(
                "select slug, name_en, name_ja from avatars where slug in ('pon-chune', 'ragussiel', 'tien') order by slug",
            )
            .fetch_all(&pool)
            .await
            .unwrap()
        };
        let expected = [
            ("pon-chune".to_owned(), "Pon & Chune".to_owned(), Some("ぽんとちゅね".to_owned())),
            ("ragussiel".to_owned(), "Ragussiel".to_owned(), Some("ラグシエル".to_owned())),
            ("tien".to_owned(), "Tien".to_owned(), Some("ティエン".to_owned())),
        ];
        assert_eq!(rows().await, expected);
        // seed 4 listed items (< TAG_MIN_ITEMS = 5) for ティエン so a delete would not be regenerated by localize,
        // plus pre-0033 localize rows with other languages/keywords and an imported control row
        sqlx::query(
            "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags)
             select id, 'item', 's', 'u', false, 'ok', now(), '{ティエン}' from generate_series(1, 4) as id",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "insert into booth_tag_translations (tag, en, ja, ko, zh, keywords, source) values
               ('ぽんとちゅね', 'ぽんとちゅね', 'ぽんとちゅね', '폰토츄네', 'ぽんとちゅね', '{k1}', 'localize'),
               ('ラグシエル', 'ラグシエル', 'ラグシエル', '라구시엘', 'ラグシエル', '{k2}', 'localize'),
               ('ティエン', 'ティエン', 'ティエン', '티엔', '提恩', '{k3}', 'localize'),
               ('import-ctrl', 'ティエン', 'ティエン', null, null, '{}', 'import')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let tags = || async {
            sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, Option<String>, Vec<String>, String)>(
                "select tag, en, ja, ko, zh, keywords, source from booth_tag_translations order by tag collate \"C\"",
            )
            .fetch_all(&pool)
            .await
            .unwrap()
        };
        sqlx::raw_sql(SQL).execute(&pool).await.unwrap();
        assert_eq!(rows().await, expected);
        let after = tags().await;
        assert_eq!(
            after,
            [
                ("import-ctrl".to_owned(), Some("ティエン".to_owned()), Some("ティエン".to_owned()), None, None, vec![], "import".to_owned()),
                ("ぽんとちゅね".to_owned(), Some("Pon & Chune".to_owned()), Some("ぽんとちゅね".to_owned()), Some("폰토츄네".to_owned()), Some("ぽんとちゅね".to_owned()), vec!["k1".to_owned()], "localize".to_owned()),
                ("ティエン".to_owned(), Some("Tien".to_owned()), Some("ティエン".to_owned()), Some("티엔".to_owned()), Some("提恩".to_owned()), vec!["k3".to_owned()], "localize".to_owned()),
                ("ラグシエル".to_owned(), Some("Ragussiel".to_owned()), Some("ラグシエル".to_owned()), Some("라구시엘".to_owned()), Some("ラグシエル".to_owned()), vec!["k2".to_owned()], "localize".to_owned()),
            ]
        );
        // second run is a no-op
        sqlx::raw_sql(SQL).execute(&pool).await.unwrap();
        assert_eq!(rows().await, expected);
        assert_eq!(tags().await, after);
    }
}
