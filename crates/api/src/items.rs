//! `/api/items`: item records by id.

use std::sync::Arc;

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::limit::Check;
use crate::{ApiError, AppState, db, limit};

/// Full records by id (favourites list, palette picks, the item dialog): `/api/items?ids=1+2+3`, at most 100.
pub(crate) async fn items_by_id(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    let ids = item_ids(q.as_deref().unwrap_or(""));
    let limit = if ids.len() == 1 { &s.limits.item } else { &s.limits.items };
    limit.check(ip)?;
    if ids.is_empty() {
        return Ok(Json(json!({ "items": [] })).into_response());
    }
    let _permit = s.acquire_search().await?;
    let items = db::load_items(&s.pool, &ids, false).await?;
    // an id the catalogue lacks may come in later, so an answer missing one is not kept
    let whole = items.len() == ids.len();
    let body = Json(json!({ "items": items }));
    Ok(if whole { ([(axum::http::header::CACHE_CONTROL, ITEMS_CACHE)], body).into_response() } else { body.into_response() })
}

/// Records are the same for everyone, and visitors come for the search: browsers and the CDN may keep them
/// 12 hours, a price or title change included (owner). The CDN keeps them only through a cache rule (docs/DEPLOY.md).
const ITEMS_CACHE: &str = "public, max-age=43200";

/// `ids=` from a raw query, at most 100. Not `Params`: that caps lists at 10, and the web proxy re-encodes the
/// separators (`,` -> `%2C`, `+` -> `%2B`), so they're split after decoding.
fn item_ids(query: &str) -> Vec<i64> {
    let raw = query.split('&').find_map(|kv| kv.strip_prefix("ids=")).unwrap_or("");
    percent_encoding::percent_decode_str(raw).decode_utf8_lossy().split([',', '+', ' ']).filter_map(|v| v.parse().ok()).take(100).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn item_ids_accept_any_separator_encoding() {
        assert_eq!(super::item_ids("ids=1,2+3%2C4%2B5%206&x=7"), [1, 2, 3, 4, 5, 6]);
        assert_eq!(super::item_ids(&format!("ids={}", (1..=150).map(|i| i.to_string()).collect::<Vec<_>>().join(","))).len(), 100);
        assert!(super::item_ids("q=1").is_empty());
    }
}
