//! The job kinds, in one place. The worker dispatches on `JobKind` with an exhaustive `match`, and the API accepts
//! only these names. Tests check the copies that cannot import it: the newest `jobs_kind_check` migration here, and
//! `web/src/components/admin/types.ts` in `web/`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Incremental,
    Details,
    Refresh,
    Sweep,
    Enrich,
    Embed,
    Localize,
}

impl JobKind {
    pub const ALL: [JobKind; 7] = [Self::Incremental, Self::Details, Self::Refresh, Self::Sweep, Self::Enrich, Self::Embed, Self::Localize];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Incremental => "incremental",
            Self::Details => "details",
            Self::Refresh => "refresh",
            Self::Sweep => "sweep",
            Self::Enrich => "enrich",
            Self::Embed => "embed",
            Self::Localize => "localize",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[cfg(test)]
mod tests {
    use super::JobKind;

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    #[test]
    fn all_lists_every_variant() {
        // `as_str` is an exhaustive match, so it has one arm per variant; ALL must be as long
        let source = include_str!("jobs.rs");
        let arms = source.lines().filter(|l| l.trim_start().starts_with("Self::") && l.contains("=> \"")).count();
        assert_eq!(JobKind::ALL.len(), arms);
    }

    #[test]
    fn names_round_trip() {
        for k in JobKind::ALL {
            assert_eq!(JobKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(JobKind::parse("x"), None);
    }

    #[test]
    fn kinds_match_the_newest_database_constraint() {
        // migrations already applied are never edited: the newest one that redefines the constraint lists every kind
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        let sql = files.iter().rev().map(|f| std::fs::read_to_string(f).unwrap()).find(|s| s.contains("add constraint jobs_kind_check")).unwrap();
        let line = sql.lines().find(|l| l.contains("add constraint jobs_kind_check")).unwrap();
        let db = line.split('\x27').skip(1).step_by(2).map(str::to_owned).collect();
        assert_eq!(sorted(JobKind::ALL.iter().map(|k| k.as_str().to_owned()).collect()), sorted(db));
    }
}
