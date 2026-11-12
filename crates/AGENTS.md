# crates/AGENTS.md: backend rules

Rules for every change to the files under this directory. The root `AGENTS.md` (rule priority,
workflow, CI, writing, commits) applies as well; numbers are in `REQUIREMENTS.md`.

## Backend

### APIs and modularity

- Clear API abstraction and module boundaries.
- No hardcoded URLs, paths or config values. Upstream fees, limits and rules are data, not code.
- No broken endpoints.

### Compatibility

- If a config format, a database or storage schema, or API behaviour changes, old data and old
  clients keep working, or a migration is provided.
- Migrations (if present) only move forward, can be rerun safely, and apply on a database that
  already holds rows.

### Logging

- One log format, with levels used for their real meaning: debug, info, warn, error.
- Do not log expected cases as errors.
- Do not swallow errors silently.
- No secrets in logs (root `AGENTS.md`, "Secrets").

### Security (if present)

- **Endpoints.** [rule: authz]
  - Every endpoint checks authorization, except one marked public where its route is defined (a
    health check, sign-in, a public API).
  - Input is validated at the boundary.
  - CSRF, CORS and rate limits are set on the endpoints that need them.
- **Errors.**
  - Errors shown to end users carry no upstream or internal detail: no vendor names, ids or raw
    messages.
  - Operators who run the upstream connection see that detail, because they need it to
    troubleshoot.
- **Sensitive values.** [rule: sensitive-values]
  - Card numbers, secrets and tokens are refused or masked at the write boundary of every
    free-text field, with a database constraint as the backstop.
  - No log, including the database's own, echoes a refused value.
- **Credentials.** Every credential and key file is loaded through one path that checks its owner
  and mode. None is read ad hoc, or taken from a plain environment variable when the others are
  files.
- **Lockouts and recovery.**
  - Every lockout or ceiling has a defined way out, such as support's reset, and that way out is
    tested.
  - An admin recovery leaves no old credential valid; the account opens only through the
    recovery's own step.
- **Account enumeration.** Unknown and known accounts get the same answers and do the same work.

### Money and state (if present)

- Every operation that moves money is idempotent and safe under concurrent requests and
  out-of-order callbacks. [rule: money-idempotent]
- Any other operation that a client, an upstream or a job may retry (a callback, a webhook, a
  queued job) is idempotent; the rest is safe under concurrent requests.
- The ledger balances to every balance it feeds, and a replay can rebuild and check it.
  [rule: ledger]
- An account or record cannot be closed while money is still in motion for it.

### Background work (if present)

- A failing or panicking background round is contained: it is logged with its job name, it raises
  an alert, and the service and the other jobs keep running.

### Operations (if present)

- Deploy production only from merged main. Any override that skips that check refuses the
  production target.
- Every production step waits for the owner's explicit go.
- Backups come with a written restore procedure. It says how to recover what happened after the
  last backup, and where the key files the restore needs are kept (never their values).

### Admin panel (if present)

- Do not over-engineer, and avoid adding maintenance burden.
- Cut unnecessary features to lower long-term maintenance cost.
- Every setting the business changes while the service runs has an admin page; API-only is not
  done. A value that changes only with a deploy stays in config.

### Performance

- Watch storage size, CPU and memory against the baseline.
- No unnecessary dependencies.

### Verification

- When needed, run and test in an isolated environment: a VM, or a network namespace with
  disposable data.
- Verify against a real database. Builds, vet and unit tests pass through defects that only a real
  database catches.
