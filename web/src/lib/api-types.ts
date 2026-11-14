// Generated from the Rust response types (crates/api/src/api_types.rs). Do not edit:
// change the Rust types, then run `UPDATE_API_TYPES=1 cargo test -p api api_types`.

export type ItemResponse = { id: number, title: string, bio: string, author_name: string, author_uri: string, author_icon: string | null, booth_category: string | null, price_jpy: number | null, smart_price: number | null, likes: number, is_adult: boolean, content_rating: ContentRating | null, quest: string | null, features: Array<string>, status: Status, published_at: string | null, scraped_at: string, original_language: string | null, avatar_slug: string | null, supported: boolean, gender: Gender | null, needs_enrichment: boolean, last_listed_at: string | null, enrich_failures: number, media: Array<string>, booth_tags: Array<string>, variations: Array<Variation>, 
/**
 * AI translations by language, plus `original_language`. Kept as raw JSON: a few rows carry stray keys, and
 * the API passes them through untouched. The site sees the documented shape.
 */
translations: Translations, linked_avatars: Array<LinkedAvatar>, categories: Array<ItemCategory>, booth_images: Array<string> | null, booth_images_complete: boolean | null, 
/**
 * delisted, but re-uploaded by the shop under this id
 */
relisted_as: number | null, };

export type ContentRating = "SFW" | "Suggestive" | "NSFW";

export type Status = "ok" | "delisted";

export type Gender = "female" | "male";

export type Variation = { id: VariationId, name: string, price_jpy: number | null, status: string, type: string, };

export type VariationId = number | string;

export type LinkedAvatar = { slug: string, en: string, ja: string | null, ko: string | null, image_url: string | null, };

export type ItemCategory = { slug: string, is_primary: boolean, };

export type Translations = { zh?: Translation, en?: Translation, ja?: Translation, ko?: Translation, original_language?: string, };

export type Translation = { title?: string, bio?: string, 
/**
 * variation name -> translated name (null when the model returned none)
 */
variations?: { [key in string]: string | null }, };

export type SearchResponse = { count: number, page: number, totalPages: number, pageSize: number, sort: string, dir: string, boothCounts: { [key in string]: number }, smartCounts: { [key in string]: number }, featureCounts: { [key in string]: number }, items: Array<ItemResponse>, semantic: SemanticStatus, 
/**
 * what the search read the query's words as: the site marks those tokens
 */
recognized: Array<Recognized>, trace: Array<TraceStep>, timing: Timing, 
/**
 * for a search that finds nothing, asked with `without=`: what it finds without each group, in order
 */
widerCounts: Array<number>, 
/**
 * the query's words matched nothing as typed, so its word matches allow one typo per long word; or a word was
 * read as the model whose name it misspells
 */
typoTolerant: boolean, };

export type SemanticStatus = "used" | "off" | "unavailable" | "skipped";

export type Recognized = { kind: RecognizedKind, slug: string, 
/**
 * as typed
 */
words: Array<string>, };

export type RecognizedKind = "model" | "category";

export type TraceStep = { step: string, ms: number, };

export type Timing = { searchMs: number, totalMs: number, };

export type Refusal = "sign_in_required" | "admins_only" | "cross_site_request" | "bad_credentials" | "too_many_attempts" | "account_disabled" | "registration_closed" | "display_name_required" | "bad_username" | "bad_email" | "password_too_short" | "name_taken" | "password_unchanged" | "wrong_current_password" | "wrong_password" | "admin_cannot_delete" | "bad_preferences" | "unknown_item" | "bad_saved_search" | "saved_search_limit" | "no_such_user" | "admin_cannot_be_disabled" | "no_such_job" | "bad_job" | "job_already_active" | "job_not_active" | "no_such_schedule" | "bad_schedule" | "bad_ai_settings" | "ai_key_required" | "ai_base_url_locked" | "ai_not_configured" | "provider_failed";
