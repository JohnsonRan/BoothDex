# Development & Review Requirements

## Scope
- Apply these rules based on the actual project. Not every project or change involves every item.
- Skip sections the project does not have (e.g. frontend, backend, i18n, theming, admin panel).
- Do not add features, abstractions, or tests just to satisfy this checklist.
- Items marked "(if present)" only apply when that feature already exists.
- This file defines rules only. Project-specific checks, gates, and numbers live in each project's own file (e.g. REQUIREMENTS.md).

---

## 1. General (applies to all changes)

### Workflow
- Plan before coding: define scope, impact, and how to verify.
- Don't be reckless, and don't drag things out.
- No over-engineering, no over-fixing, no cutting corners.
- Make sure a change is a real improvement, not over-engineering or a pile of scattered mess.

### When unsure
- If you can't tell whether something is intentional, or a change goes beyond the agreed scope, don't decide on your own and don't change it.
- Write the question down in an open-questions list and keep working on other unblocked tasks.
- Ask the owner all open questions together at a good stopping point.
- Only ask right away if the question blocks all remaining work.

### Waiting
- Don't sit idle on one long wait (build, test, KVM, CI, subagent, owner reply).
- Set a timeout, schedule a wake-up to check back regularly, and do other work in the meantime.
- If something is still stuck after the timeout, record it with its current state and move on.

### Reporting
- Report what was actually run: commands, tests, and their results.
- If something was not verified, say so clearly. Never claim "tested" without running it.

### Code quality
- Readability first; keep logic clear.
- No spaghetti code.
- Remove useless comments, duplicate code, and leftovers (dead code, debug output, temp files).
- Use modern practices, but not just for the sake of being new.
- When rewriting, match the existing code style, unless the owner has set a different convention. Then follow that convention.

### Impact & edge cases
- Check how the change affects other areas.
- Watch for intentional design choices; don't "fix" them as bugs (see "When unsure").
- Handle edge cases reasonably; no excessive defensive code.

### Secrets
- Never write keys, tokens, or credentials to logs.
- Never store them in client-side storage (localStorage, sessionStorage, IndexedDB, cookies without proper flags).
- Never hardcode them in source.

### Performance baseline
- Before a rewrite or optimization, measure a baseline.
- After the change, results must not be worse than the baseline.

### Tests (if present)
- Keep tests lean and focused on critical paths.
- Each project must set a test time limit and measure against it. The actual numbers go in the project's own file.

### Model usage
- Important design / review work: Opus, Sonnet.
- Minor tasks: use sol or similar to save tokens.
- Never trust sol output directly; verify it before using it.

---

## 2. Frontend (if present)

### Components & modularity
- Unified components; don't reinvent existing ones.
- Finish any existing modularization; no half-done work (e.g. theme colors or i18n only partly extracted, the rest hardcoded).
- No hardcoded text, colors, or sizes.

### Consistency
- Unified animations (if present): duration, easing, triggers.
- Unified visual style.
- Unified notification levels with correct meaning (if present): info / warning / error / success.

### i18n (if present)
- No missing translations, no hardcoded strings.
- Consistent key naming; no duplicate or unused keys.

### Usability
- Generic and easy to use.
- Compatible across browsers / screen sizes / themes (if present).

### Accessibility
- All actions work with keyboard only, with visible focus.
- Text and UI meet WCAG AA contrast (4.5:1 for normal text), in every theme (if present).

### Performance
- Watch bundle size, render cost, and memory, compared against the baseline.
- No unnecessary dependencies.

---

## 3. Backend (if present)

### APIs & modularity
- Clear API abstraction and module boundaries.
- No hardcoded URLs, paths, or config values.
- No broken endpoints.

### Compatibility
- If config format, database / storage schema, or API behavior changes, old data and old clients must keep working, or a migration must be provided.

### Logging
- Consistent
