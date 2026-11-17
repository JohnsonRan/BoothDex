-- Whether the sign-in or registration that made a session passed a Turnstile check: only such a session gives a
-- visitor today's rate limits (crates/api/src/pass.rs). A session made before this column, while the check was off,
-- or while Cloudflare could not judge the token and the sign-in was let through, is unchecked.

alter table sessions add column if not exists turnstile_passed boolean not null default false;
