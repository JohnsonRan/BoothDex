//! `/api/palette`: the Ctrl+K search.

use std::sync::Arc;

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::params::Params;
use crate::search_request::{SearchRequest, Sort};
use crate::{ApiError, AppState, db, limit};

/// Ctrl+K: one query across items, avatars, authors, tags and categories.
pub(crate) async fn palette(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    s.limits.palette.check(ip)?;
    let raw = q.unwrap_or_default();
    let user = SearchRequest::from(&Params::parse(&raw));
    let Some(text) = user.q else {
        return Ok(Json(json!({ "items": [], "avatars": [], "authors": [], "tags": [], "categories": [] })).into_response());
    };
    let _permit = s.acquire_search().await?;
    let small = Params::parse(&format!("limit=5&{raw}"));
    let needle = crate::text::fold(&text);
    let snap = s.snap();
    let categories: Vec<Value> = snap.categories["categories"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|p| std::iter::once(p).chain(p["children"].as_array().into_iter().flatten()))
        .filter(|c| ["en", "ja", "ko", "zh"].iter().any(|k| c[*k].as_str().is_some_and(|n| crate::text::fold(n).contains(&needle))))
        .take(5)
        .map(|c| json!({ "slug": c["slug"], "en": c["en"], "ja": c["ja"], "ko": c["ko"], "zh": c["zh"] }))
        .collect();
    // items honour the caller's adult setting (default safe) like the main search
    let items_req = SearchRequest { q: Some(text), adult: user.adult, sort: Sort::Relevant, page_size: 8, ..SearchRequest::from(&Params::default()) };
    let now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&items_req, now, None, None, false))?;
    let items: Vec<Value> = db::load_items(&s.pool, &r.ids, true)
        .await?
        .into_iter()
        .map(|it| {
            let image = it.booth_images.as_ref().and_then(|imgs| imgs.first().cloned())
                .or_else(|| it.media.first().cloned());
            json!({
                "id": it.id, "title": it.title, "author_name": it.author_name,
                "price_jpy": it.price_jpy, "smart_price": it.smart_price,
                "image": image,
            })
        })
        .collect();
    Ok(Json(json!({
        "items": items,
        "itemCount": r.count,
        "avatars": s.snap().avatars.find(&small)["items"],
        "authors": s.snap().authors.find(&small)["items"],
        "tags": s.snap().tags.find(&small)["items"],
        "categories": categories,
    }))
    .into_response())
}
