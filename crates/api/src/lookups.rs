//! The public catalogue lookups (`/api/avatars`, `/api/authors`, `/api/tags`, `/api/booth-tags`, `/api/categories`):
//! the handler behind each list and the rate limit of the group.

use std::sync::Arc;

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::params::Params;
use crate::snapshot::{Lookup, Snapshot};
use crate::{AppState, limit};

pub(crate) fn lookup(f: fn(&Snapshot) -> &Lookup) -> impl Fn(State<Arc<AppState>>, RawQuery) -> std::future::Ready<Json<Value>> + Clone {
    move |State(s), RawQuery(q)| std::future::ready(Json(f(&s.snap()).find(&Params::parse(q.as_deref().unwrap_or("")))))
}

pub(crate) async fn lookup_rate_limit(
    State(s): State<Arc<AppState>>,
    limit::ClientIp(ip): limit::ClientIp,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if let Err(r) = s.limits.lookup.check(ip) {
        return r.into_response();
    }
    next.run(req).await
}
