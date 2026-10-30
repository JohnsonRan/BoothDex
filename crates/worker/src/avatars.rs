//! Item -> avatar links by exact name matching against the avatar dictionary.
//! Measured on 10,089 archived items against VRCatalogue's own links (title + tags + variation names):
//! precision 0.87, recall 0.88. Descriptions are left out: they mention many avatars in passing and cut
//! precision to 0.74.

pub struct Matcher {
    keys: Vec<(String, Vec<Name>)>,
}

enum Name {
    /// ASCII names match on word boundaries ("Lime" must not match "Limelight")
    Word(String),
    /// CJK names match as substrings
    Sub(String),
}

fn norm(s: &str) -> String {
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
}
