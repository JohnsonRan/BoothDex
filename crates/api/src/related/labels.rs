//! Tag dictionary labels, avatar-name filtering and respelling/plural matching for related searches.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::expansion::Equivalents;
use crate::tag_words::TagRow;
use crate::text::{compact, fold};

/// The tag dictionary's rows (`booth_tag_translations`) of the tags a related search can be: those folding to a tag on
/// enough listed items (`Equivalents::id`).
#[derive(Default)]
pub(crate) struct TagLabels {
    /// per canonical tag (`Equivalents::name`), its row in `rows`
    tags: FxHashMap<String, u32>,
    rows: Vec<Label>,
    /// per form (`forms`) of a compacted tag, value or keyword, the rows holding it: the tags that respell a query's
    /// words, found without reading every row
    spelt: FxHashMap<Box<str>, Vec<u32>>,
}

/// A tag's values in the languages of `LANGUAGES`, and its keywords.
struct Label {
    values: [Option<Box<str>>; 4],
    keywords: Box<[Box<str>]>,
}

impl Label {
    /// Its keywords count: search finds the tag's items by them as by its values (tag_words.rs).
    fn spellings(&self) -> impl Iterator<Item = &str> {
        self.values.iter().flatten().chain(self.keywords.iter()).map(|v| &**v)
    }
}

impl TagLabels {
    pub(crate) fn new(rows: &[TagRow], tags: &Equivalents) -> Self {
        // pick one row per canonical tag, preferring an exact spelling match when case/width variants both exist
        let mut chosen: FxHashMap<&str, (&TagRow, bool)> = FxHashMap::default();
        for r in rows {
            let Some(t) = tags.id(&fold(&r.tag)) else { continue };
            let canonical = tags.name(t);
            let exact = r.tag == canonical;
            match chosen.get(canonical) {
                Some((_, true)) => {}
                Some((_, false)) if !exact => {}
                _ => {
                    chosen.insert(canonical, (r, exact));
                }
            }
        }
        let mut out = Self::default();
        for (canonical, (r, _)) in chosen {
            let row = out.rows.len() as u32;
            let values = std::array::from_fn(|k| r.values[k].as_deref().map(Box::from));
            let label = Label { values, keywords: r.keywords.iter().map(|k| Box::from(k.as_str())).collect() };
            for spelling in std::iter::once(r.tag.as_str()).chain(label.spellings()).map(compact) {
                for form in forms(&spelling) {
                    let rows = out.spelt.entry(form.into()).or_default();
                    if rows.last() != Some(&row) {
                        rows.push(row);
                    }
                }
            }
            out.tags.insert(canonical.to_owned(), row);
            out.rows.push(label);
        }
        out
    }

    fn row(&self, tag: &str) -> Option<&Label> {
        self.tags.get(tag).map(|&row| &self.rows[row as usize])
    }

    /// The tag in the language `LANGUAGES[k]`.
    pub(super) fn label(&self, tag: &str, k: usize) -> Option<&str> {
        self.row(tag)?.values[k].as_deref()
    }

    /// Whether `like` accepts some value or keyword of the tag.
    pub(super) fn reads_as(&self, tag: &str, like: impl Fn(&str) -> bool) -> bool {
        self.row(tag).is_some_and(|l| l.spellings().any(like))
    }

    /// The forms (`forms`) of the values in `LANGUAGES[k]` of the tags with a tag, value or keyword that respells one
    /// of `phrases`: what that language calls them.
    pub(super) fn translations<'p>(&self, k: usize, phrases: impl IntoIterator<Item = &'p str>) -> FxHashSet<String> {
        let mut out = FxHashSet::default();
        for phrase in phrases.into_iter().map(compact) {
            for &row in forms(&phrase).filter_map(|form| self.spelt.get(form)).flatten() {
                if let Some(value) = self.rows[row as usize].values[k].as_deref().map(compact) {
                    out.extend(forms(&value).map(str::to_owned));
                }
            }
        }
        out
    }
}

/// Avatar names (`fold`ed) that a tag holds when it names an avatar (`Kikyo (桔梗)`, `桔梗対応`): the tag then says
/// which avatar an item fits, which the search reads from the model's name already.
#[derive(Default)]
pub(crate) struct AvatarNames {
    exact: FxHashSet<String>,
    /// single-token ASCII alphanumeric names of at least 3 characters, matched on ASCII alphanumeric boundaries
    words: FxHashSet<String>,
    /// multi-word or punctuated ASCII names of at least 3 characters (`rusk dragon`, `aoi-chan`, `noshime:re`),
    /// matched on ASCII alphanumeric boundaries
    phrases: Vec<String>,
    /// other names of at least 2 characters, found anywhere, by their first two characters
    inside: FxHashMap<(char, char), Vec<Vec<char>>>,
}

impl AvatarNames {
    pub(crate) fn new<'n>(names: impl IntoIterator<Item = &'n str>) -> Self {
        let mut out = Self::default();
        let mut phrases = FxHashSet::default();
        for name in names.into_iter().map(fold).filter(|n| !n.is_empty()) {
            let chars: Vec<char> = name.chars().collect();
            if name.is_ascii() && name.len() >= 3 {
                if name.bytes().all(|b| b.is_ascii_alphanumeric()) {
                    out.words.insert(name.clone());
                } else {
                    phrases.insert(name.clone());
                }
            } else if !name.is_ascii() && chars.len() >= 2 {
                out.inside.entry((chars[0], chars[1])).or_default().push(chars);
            }
            out.exact.insert(name);
        }
        out.phrases = phrases.into_iter().collect();
        out
    }

    /// Whether the folded tag names an avatar.
    pub(crate) fn named_in(&self, tag: &str) -> bool {
        let chars: Vec<char> = tag.chars().collect();
        self.exact.contains(tag)
            || tag.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| self.words.contains(w))
            || self.phrases.iter().any(|p| {
                tag.match_indices(p.as_str()).any(|(i, _)| {
                    let before = tag[..i].chars().next_back();
                    let after = tag[i + p.len()..].chars().next();
                    !before.is_some_and(|c| c.is_ascii_alphanumeric()) && !after.is_some_and(|c| c.is_ascii_alphanumeric())
                })
            })
            || (0..chars.len().saturating_sub(1))
                .any(|i| self.inside.get(&(chars[i], chars[i + 1])).is_some_and(|names| names.iter().any(|n| chars[i..].starts_with(n))))
    }
}

/// The same word spelt again: equal once case, width, spaces and symbols are gone, or but for a plural `s` or `es`
/// (gothic girl / gothic girls, eye / eyes, dress / dresses). Unlike `expansion::same_word`, one holding the other does
/// not count: ミニスカート is a related search for スカート.
pub(super) fn respelling(a: &str, b: &str) -> bool {
    let (a, b) = (compact(a), compact(b));
    forms(&a).any(|x| forms(&b).any(|y| x == y))
}

/// A compacted word, and the word without an English plural `s` or `es` (when the remaining stem is at least 2 bytes
/// and ends in an ASCII lowercase letter, so empty strings, `"s"`/`"es"`, `"as"`, `"10s"` and non-Latin stems are not
/// stripped).
pub(super) fn forms(s: &str) -> impl Iterator<Item = &str> {
    if s.is_empty() {
        return [None, None, None].into_iter().flatten();
    }
    let stem = |suffix: &str| s.strip_suffix(suffix).filter(|rest| rest.len() >= 2 && rest.as_bytes().last().is_some_and(u8::is_ascii_lowercase));
    [Some(s), stem("s"), stem("es")].into_iter().flatten()
}
