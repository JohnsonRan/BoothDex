//! `AvatarKit`: the `avatars` table loaded once for both `localize` and `enrich`.

use std::collections::HashMap;

use anyhow::Result;
use sqlx::PgPool;

/// The avatars table as the AI jobs read it.
pub(super) struct AvatarKit {
    pub(super) matcher: crate::avatars::Matcher,
    pub(super) names: HashMap<String, crate::ai::AvatarName>,
    pub(super) forms: HashMap<String, Vec<String>>,
    pub(super) romaji: HashMap<String, String>,
    pub(super) japanese: Vec<String>,
}

impl AvatarKit {
    /// The avatar names `text` (a tag or a title) holds: for each avatar the matcher finds in it, the names it writes
    /// and the avatar's Japanese name and romaji, each NFKC and lowercase, deduplicated in first-seen order.
    pub(super) fn held_names(&self, text: &str) -> Vec<crate::ai::HeldName> {
        let folded = crate::avatars::norm(text);
        self.matcher
            .find(text)
            .iter()
            .filter_map(|slug| {
                let name = self.names.get(slug)?;
                let written = self.forms.get(slug)?.iter().map(|f| crate::avatars::norm(f)).filter(|f| folded.contains(f.as_str()));
                let mut forms: Vec<String> = Vec::new();
                for f in written.chain(name.ja.iter().chain([&name.romaji]).map(|f| crate::avatars::norm(f))) {
                    if !forms.contains(&f) {
                        forms.push(f);
                    }
                }
                let latin = forms.iter().filter(|f| crate::ai::latin_letters(f)).cloned().collect();
                Some(crate::ai::HeldName { forms, latin })
            })
            .collect()
    }
}

/// `(slug, name_en, name_ja, name_ko, aliases)` read from `avatars`.
type AvatarRow = (String, String, Option<String>, Option<String>, Vec<String>);

/// Reads `avatars` once and builds both the name-only `AvatarKit` (for title/tag checks) and the alias-including
/// `Matcher` (for item-avatar linking).
pub(super) async fn load_avatars(pool: &PgPool) -> Result<(AvatarKit, crate::avatars::Matcher)> {
    let avatars: Vec<AvatarRow> =
        sqlx::query_as("select slug, name_en, name_ja, name_ko, aliases from avatars order by slug").fetch_all(pool).await?;
    let forms: HashMap<String, Vec<String>> =
        avatars.iter().map(|(s, en, ja, ko, _)| (s.clone(), [Some(en), ja.as_ref(), ko.as_ref()].into_iter().flatten().cloned().collect())).collect();
    let matcher = crate::avatars::Matcher::new(forms.iter().map(|(s, f)| (s.clone(), f.clone())));
    let links = crate::avatars::Matcher::new(avatars.iter().map(|(s, _, _, _, aliases)| {
        let mut all = forms[s].clone();
        all.extend(aliases.iter().cloned());
        (s.clone(), all)
    }));
    let romaji = avatars
        .iter()
        .flat_map(|(_, en, ja, ko, _)| [Some(en), ja.as_ref(), ko.as_ref()].into_iter().flatten().map(move |n| (n.trim().to_lowercase(), en.clone())))
        .collect();
    let names: HashMap<String, crate::ai::AvatarName> =
        avatars.into_iter().map(|(slug, romaji, ja, _, _)| (slug, crate::ai::AvatarName { ja, romaji })).collect();
    let japanese = crate::ai::AvatarName::japanese_names(names.values());
    Ok((AvatarKit { matcher, names, forms, romaji, japanese }, links))
}
