pub mod booth;
pub mod booth_client;
pub mod embed;
pub mod settings;

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
