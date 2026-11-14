//! The public catalogue lookups (`/api/avatars`, `/api/authors`, `/api/tags`, `/api/booth-tags`, `/api/categories`):
//! the handler behind each list and the rate limit of the group.

use std::sync::Arc;

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::limit::Check;
use crate::params::Params;
use crate::snapshot::{Lookup, Snapshot};
use crate::{AppState, limit};

/// The lists change only when the catalogue snapshot is rebuilt: normally at most once per 5 minutes (snapshot.rs;
/// an admin "reload now" can rebuild sooner). A browser or the CDN may keep an answer 5 minutes, so a repeated
/// typeahead word or picker opening is not asked again; after a forced reload a kept list lags up to that long.
const LOOKUP_CACHE: &str = "public, max-age=300";

fn cached(body: Value) -> Response {
    ([(axum::http::header::CACHE_CONTROL, LOOKUP_CACHE)], Json(body)).into_response()
}

pub(crate) fn lookup(f: fn(&Snapshot) -> &Lookup) -> impl Fn(State<Arc<AppState>>, RawQuery) -> std::future::Ready<Response> + Clone {
    move |State(s), RawQuery(q)| std::future::ready(cached(f(&s.snap()).find(&Params::parse(q.as_deref().unwrap_or("")))))
}

pub(crate) async fn categories(State(s): State<Arc<AppState>>) -> Response {
    cached(s.snap().categories.clone())
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

#[cfg(test)]
mod tests {
    use axum::extract::{RawQuery, State};
    use axum::http::header::CACHE_CONTROL;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn lookups_may_be_kept_for_a_few_minutes() {
        let state = crate::state::test_state();
        let tags = super::lookup(|s| &s.tags)(State(state.clone()), RawQuery(Some("q=hair".into()))).await.into_response();
        assert_eq!(tags.headers()[CACHE_CONTROL], super::LOOKUP_CACHE);
        let categories = super::categories(State(state)).await.into_response();
        assert_eq!(categories.headers()[CACHE_CONTROL], super::LOOKUP_CACHE);
    }
}
