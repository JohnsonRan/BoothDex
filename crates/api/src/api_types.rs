//! TypeScript types for the site, generated from the response types (`web/src/lib/api-types.ts`).
//!
//! `cargo test` fails when that file is out of date; `UPDATE_API_TYPES=1 cargo test -p api api_types` rewrites it.

use ts_rs::{Config, TS};

use crate::{db, error, search};

const OUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/src/lib/api-types.ts");

/// Every exported type, dependencies included, in one file (no imports between generated files).
fn generate() -> String {
    // i64/u64/usize ids and counts are plain JSON numbers, well inside 2^53
    let cfg = Config::new().with_large_int("number");
    let decls = [
        db::ItemResponse::decl(&cfg),
        db::ContentRating::decl(&cfg),
        db::Status::decl(&cfg),
        db::Gender::decl(&cfg),
        db::Variation::decl(&cfg),
        db::VariationId::decl(&cfg),
        db::LinkedAvatar::decl(&cfg),
        db::ItemCategory::decl(&cfg),
        db::Translations::decl(&cfg),
        db::Translation::decl(&cfg),
        search::SearchResponse::decl(&cfg),
        search::SemanticStatus::decl(&cfg),
        search::Recognized::decl(&cfg),
        search::RecognizedKind::decl(&cfg),
        search::TraceStep::decl(&cfg),
        search::Timing::decl(&cfg),
        error::Refusal::decl(&cfg),
    ];
    let mut out = String::from(
        "// Generated from the Rust response types (crates/api/src/api_types.rs). Do not edit:\n\
         // change the Rust types, then run `UPDATE_API_TYPES=1 cargo test -p api api_types`.\n",
    );
    for d in decls {
        out.push_str("\nexport ");
        out.push_str(&d);
        out.push('\n');
    }
    out
}

#[test]
fn api_types_are_current() {
    let fresh = generate();
    if std::env::var_os("UPDATE_API_TYPES").is_some() {
        std::fs::write(OUT, &fresh).unwrap();
        return;
    }
    let current = std::fs::read_to_string(OUT).unwrap_or_default().replace("\r\n", "\n");
    assert!(current == fresh, "web/src/lib/api-types.ts is out of date: run `UPDATE_API_TYPES=1 cargo test -p api api_types`");
}
