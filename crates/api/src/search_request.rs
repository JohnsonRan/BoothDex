//! A search as typed values, read once from the query string. Every default, filter and clamp of the search
//! parameters is applied here, so `Catalog::search` only matches on them.

use chrono::NaiveDate;

use crate::catalog::FEATURES;
use crate::params::Params;
use crate::text::fold_width;

#[derive(Clone)]
pub struct SearchRequest {
    /// never blank
    pub q: Option<String>,
    pub view: View,
    /// female or male; applies in the avatar view
    pub gender: Option<String>,
    pub adult: Adult,
    pub avatars: Vec<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub btags: Vec<String>,
    pub bc: Vec<String>,
    pub sc: Vec<String>,
    /// `sc` matches primary categories only
    pub scp: bool,
    pub extags: Vec<String>,
    pub exbtags: Vec<String>,
    /// `if`, known features only
    pub features: Vec<String>,
    pub free: bool,
    pub pmin: i32,
    pub pmax: i32,
    pub range: Option<Range>,
    pub sort: Sort,
    pub asc: bool,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Clone, Copy)]
pub enum View {
    Items,
    Avatars(Support),
    Favourites,
}

#[derive(Clone, Copy)]
pub enum Support {
    Supported,
    Unsupported,
    All,
}

#[derive(Clone, Copy)]
pub enum Adult {
    Safe,
    Include,
    Only,
    Suggestive,
}

#[derive(Clone, Copy)]
pub enum Range {
    Days(i64),
    Custom { from: Option<NaiveDate>, to: Option<NaiveDate> },
}

#[derive(Clone, Copy)]
pub enum Sort {
    /// by relevance when there is a text query, else trending
    Relevant,
    Newest,
    Price,
    Likes,
    Title,
    Trending,
}

impl From<&Params> for SearchRequest {
    fn from(p: &Params) -> Self {
        let date = |k: &str| p.get(k).and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
        Self {
            q: p.get("q").filter(|q| !q.trim().is_empty()),
            view: match p.get("v").as_deref() {
                Some("favourites") => View::Favourites,
                Some("avatars") => View::Avatars(match p.get("asup").as_deref() {
                    Some("all") => Support::All,
                    Some("unsupported") => Support::Unsupported,
                    _ => Support::Supported,
                }),
                _ => View::Items,
            },
            gender: p.get("gender").filter(|g| g == "female" || g == "male"),
            adult: match p.get("adult").as_deref() {
                Some("include") => Adult::Include,
                Some("adult_only") => Adult::Only,
                Some("suggestive") => Adult::Suggestive,
                _ => Adult::Safe,
            },
            avatars: p.list("avatars").to_vec(),
            authors: p.list("authors").to_vec(),
            tags: p.list("tags").to_vec(),
            btags: p.list("btags").to_vec(),
            bc: p.list("bc").to_vec(),
            sc: p.list("sc").to_vec(),
            scp: p.get("scp").as_deref() == Some("1"),
            extags: p.list("extags").to_vec(),
            exbtags: p.list("exbtags").to_vec(),
            features: p.list("if").iter().filter(|f| FEATURES.contains(&f.as_str())).cloned().collect(),
            free: p.get("free").as_deref() == Some("1"),
            pmin: p.num("pmin").unwrap_or(0).max(0),
            pmax: p.num("pmax").unwrap_or(0),
            range: match p.get("range").as_deref() {
                Some("7d") => Some(Range::Days(7)),
                Some("30d") => Some(Range::Days(30)),
                Some("90d") => Some(Range::Days(90)),
                Some("1y") => Some(Range::Days(365)),
                Some("custom") => Some(Range::Custom { from: date("from"), to: date("to") }),
                _ => None,
            },
            sort: match p.get("sort").as_deref() {
                Some("relevant") | None => Sort::Relevant,
                Some("newest") => Sort::Newest,
                Some("price") => Sort::Price,
                Some("likes") => Sort::Likes,
                Some("title") => Sort::Title,
                _ => Sort::Trending,
            },
            asc: p.get("dir").as_deref() == Some("asc"),
            page: p.num("page").unwrap_or(1),
            page_size: p.num("pageSize").unwrap_or(20).clamp(1, 100),
        }
    }
}

/// What a whole text query names when it refers to one item (`item_reference`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemReference {
    /// a BOOTH item link: it names the item whether or not the catalogue has it
    Link(i64),
    /// a number of at least `MIN_ITEM_DIGITS` digits: it names an item only when the catalogue has that id
    Number(i64),
}

/// Shorter numbers are prices and years ("1000", "2024"), searched as text.
const MIN_ITEM_DIGITS: usize = 5;

/// Reads the whole query, trimmed and folded (`fold_width`), as a BOOTH item link or an item number. A link is
/// `booth.pm/<lang>/items/<id>` or `<shop>.booth.pm/items/<id>`, with an optional `http://` or `https://`, `www.`,
/// trailing `/`, query string and fragment; any other path or host is text.
pub fn item_reference(q: &str) -> Option<ItemReference> {
    let q = fold_width(q.trim());
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if q.len() >= MIN_ITEM_DIGITS && digits(&q) {
        return q.parse().ok().map(ItemReference::Number);
    }
    let rest = q.strip_prefix("https://").or_else(|| q.strip_prefix("http://")).unwrap_or(&q);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let path: Vec<&str> = rest.strip_suffix('/').unwrap_or(rest).split('/').collect();
    let label = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let id = match path.as_slice() {
        ["booth.pm", lang, "items", id] if label(lang) => id,
        [host, "items", id] if host.strip_suffix(".booth.pm").is_some_and(label) => id,
        _ => return None,
    };
    if !digits(id) {
        return None;
    }
    id.parse().ok().map(ItemReference::Link)
}

/// The filter parameters a `without=` group may name. The text query, the view, the order and the page never
/// change in a wider search, so its text and meaning-based matches are the original search's.
const FILTERS: [&str; 19] = [
    "authors", "avatars", "sc", "scp", "bc", "btags", "exbtags", "tags", "extags", "if", "adult", "free", "pmin", "pmax", "range", "from",
    "to", "gender", "asup",
];
/// One group per clause of the site's filter row (search.rs).
const MAX_WITHOUT: usize = 8;

/// `without=` from a raw query: groups of parameter names, a `+` (or a space) between groups and a `,` between
/// names. Read like `item_ids` (items.rs): split after decoding, as the web proxy re-encodes the separators. At
/// most 8 groups; a name outside `FILTERS` is dropped, and its group stays in place so the answers keep the order.
pub fn without_groups(query: &str) -> Vec<Vec<&'static str>> {
    let Some(raw) = query.split('&').find_map(|kv| kv.strip_prefix("without=")) else { return Vec::new() };
    let raw = percent_encoding::percent_decode_str(raw).decode_utf8_lossy();
    raw.split(['+', ' '])
        .take(MAX_WITHOUT)
        .map(|group| group.split(',').filter_map(|name| FILTERS.iter().find(|f| **f == name).copied()).collect())
        .collect()
}

/// The raw query with the parameters in `names` left out.
pub fn query_without(query: &str, names: &[&str]) -> String {
    query
        .split('&')
        .filter(|kv| {
            let key = kv.split_once('=').map_or(*kv, |(k, _)| k);
            !names.contains(&percent_encoding::percent_decode_str(key).decode_utf8_lossy().as_ref())
        })
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(query: &str) -> SearchRequest {
        SearchRequest::from(&Params::parse(query))
    }

    #[test]
    fn a_query_with_no_sort_sorts_by_relevance() {
        assert!(matches!(request("q=dress").sort, Sort::Relevant));
        assert!(matches!(request("sort=relevant").sort, Sort::Relevant));
        assert!(matches!(request("q=dress&sort=newest").sort, Sort::Newest));
        assert!(matches!(request("q=dress&sort=nonsense").sort, Sort::Trending));
    }

    #[test]
    fn a_booth_item_link_or_a_long_number_is_an_item_reference() {
        use ItemReference::{Link, Number};
        for (q, expected) in [
            ("https://booth.pm/ja/items/8962830", Link(8962830)),
            ("http://booth.pm/zh-cn/items/8962830/", Link(8962830)),
            ("booth.pm/ko/items/8962830", Link(8962830)),
            ("https://www.booth.pm/en/items/8962830?srsltid=abc#gallery", Link(8962830)),
            ("https://shop-name.booth.pm/items/7164608", Link(7164608)),
            ("  HTTPS://Shop.BOOTH.pm/items/123/#top ", Link(123)),
            ("8962830", Number(8962830)),
            ("１２３４５", Number(12345)),
        ] {
            assert_eq!(item_reference(q), Some(expected), "{q}");
        }
        for q in [
            "2024",
            "1000",
            "skirt 8962830",
            "8962830 skirt",
            "https://booth.pm/ja/items/8962830 skirt",
            "https://booth.pm/ja/browse/8962830",
            "https://booth.pm/items/8962830",
            "https://booth.pm//items/8962830",
            "https://booth.pm/ja/items/8962830/images",
            "https://booth.pm/ja/items/89x62830",
            "https://booth.pm/ja/items/+123",
            "https://booth.pm/ja/items/-1",
            "https://shop.booth.pm/items/+8962830",
            "https://a.b.booth.pm/items/8962830",
            "https://booth.pm.example.com/ja/items/8962830",
            "https://example.com/ja/items/8962830",
            "https://example.com/?.booth.pm/items/8962830",
            "ftp://booth.pm/ja/items/8962830",
            "99999999999999999999",
        ] {
            assert_eq!(item_reference(q), None, "{q}");
        }
    }

    #[test]
    fn without_reads_groups_in_order_and_only_filter_names() {
        assert_eq!(without_groups("sc=a&without=sc+pmin,pmax+if"), [vec!["sc"], vec!["pmin", "pmax"], vec!["if"]]);
        // the web proxy re-encodes the separators
        assert_eq!(without_groups("without=sc%2Bpmin%2Cpmax"), [vec!["sc"], vec!["pmin", "pmax"]]);
        assert_eq!(without_groups("without=q,sc+page+nonsense"), [vec!["sc"], vec![], vec![]]);
        assert_eq!(without_groups(&format!("without={}", ["sc"; 12].join("+"))).len(), MAX_WITHOUT);
        assert!(without_groups("q=sc").is_empty());
        assert_eq!(query_without("q=a&pmin=5&sc=x&pmax=9", &["pmin", "pmax"]), "q=a&sc=x");
    }
}
