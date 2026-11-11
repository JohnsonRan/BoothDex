-- a disabled account can't sign in; disabling it also deletes its sessions
alter table users add column disabled_at timestamptz;
-- set by every new session, so the admin console can tell which accounts are still in use
alter table users add column last_sign_in_at timestamptz;
-- a stable handle for one session, so the account and the admin console can sign it out (token_hash stays secret)
alter table sessions add column id bigserial unique;
