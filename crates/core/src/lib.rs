pub mod booth;
pub mod booth_client;
pub mod embed;
pub mod jobs;
pub mod settings;

/// AI enrichment gives up on an item after this many failed attempts (`items.enrich_failures`); the worker's queue skips
/// it from then on, and the admin console lists it with a retry that sets the count back to 0.
pub const ENRICH_ATTEMPTS: i16 = 3;

/// The version of the prompt the `localize` job translates the taxonomy names with (worker `ai::LOCALIZE_SYSTEM`).
/// Raise it when that prompt changes how names are worded: the API then queues a forced `localize` (startup and a
/// saved AI setting), and the worker records the version in `app_settings` (`localize_state`) once such a run has
/// translated every name. 2: 虚拟形象 for an avatar and Unity's terms (owner ruling 22, 2026-11-21); the names
/// translated before have no recorded version.
pub const LOCALIZE_PROMPT_VERSION: i64 = 2;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Tells the API the catalogue changed; it rebuilds its in-memory snapshot on the next reload check.
pub async fn bump_catalog_version(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("update catalog_meta set version = version + 1, changed_at = now()").execute(pool).await?;
    Ok(())
}

pub fn urlencode(s: &str) -> String {
    const ENCODE: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-').remove(b'_').remove(b'.').remove(b'~');
    percent_encoding::utf8_percent_encode(s, ENCODE).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding() {
        assert_eq!(urlencode(""), "");
        assert_eq!(urlencode("AZaz09-_.~"), "AZaz09-_.~");
        assert_eq!(urlencode("衣装 x"), "%E8%A1%A3%E8%A3%85%20x");
        assert_eq!(urlencode("衣装 x+y&z=1/#?%"), "%E8%A1%A3%E8%A3%85%20x%2By%26z%3D1%2F%23%3F%25");
    }
}
