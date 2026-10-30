//! Query-string parsing compatible with VRCatalogue URLs: list values are joined with a literal `+`
//! and spaces inside values are `%20`, so standard form decoding (which maps `+` to space) can't be used.

use std::collections::HashMap;

use percent_encoding::percent_decode_str;

const MAX_LIST: usize = 10;
const MAX_VALUE_LEN: usize = 100;

#[derive(Default, Debug)]
pub struct Params(HashMap<String, Vec<String>>);

impl Params {
    pub fn parse(query: &str) -> Self {
        let mut map = HashMap::new();
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let values = v
                .split('+')
                .map(|s| percent_decode_str(s).decode_utf8_lossy().trim().chars().take(MAX_VALUE_LEN).collect::<String>())
                .filter(|s| !s.is_empty())
                .take(MAX_LIST)
                .collect();
            // first occurrence wins, like URLSearchParams.get
            map.entry(percent_decode_str(k).decode_utf8_lossy().into_owned()).or_insert(values);
        }
        Self(map)
    }

    pub fn list(&self, key: &str) -> &[String] {
        self.0.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Single value; a `+` inside it is kept (e.g. free-text search).
    pub fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).filter(|v| !v.is_empty()).map(|v| v.join(" "))
    }

    pub fn num<T: std::str::FromStr>(&self, key: &str) -> Option<T> {
        self.get(key).and_then(|v| v.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::Params;

    #[test]
    fn plus_separates_and_percent20_is_space() {
        let p = Params::parse("avatars=manuka+shinano&btags=3D%20Model&q=gothic+dress&avatars=ignored&pmin=100");
        assert_eq!(p.list("avatars"), ["manuka", "shinano"]);
        assert_eq!(p.list("btags"), ["3D Model"]);
        assert_eq!(p.get("q").as_deref(), Some("gothic dress"));
        assert_eq!(p.num::<i32>("pmin"), Some(100));
        assert!(p.list("missing").is_empty());
    }

    #[test]
    fn caps_list_length() {
        let q = format!("tags={}", (0..20).map(|i| i.to_string()).collect::<Vec<_>>().join("+"));
        assert_eq!(Params::parse(&q).list("tags").len(), 10);
    }
}
