# AGENTS.md: development rules

Every agent that plans, writes or reviews a change in this repository follows this file and the
files it names.

## Rule priority

When two rules disagree, the higher one wins. Within a level, the more specific rule wins over the
general one, and the owner's newer statement wins over an older one.

1. **Hard limits.** Nothing below overrides these:
   - no secret or card number in logs, client storage or source;
   - no production step without the owner's explicit go;
   - no test traffic to a real upstream unless the owner allows it;
   - no AI attribution (when an upstream project requires AI disclosure, stop and ask the owner;
     write no disclosure yourself);
   - no bypassing signing, review or a gate by switching it off (quarantining a flaky test with an
     owner and a date is the one allowed exception).
2. **The owner's explicit instruction for the current task.**
3. **The owner's recorded rulings.** A ruled behaviour is not a finding and is not asked again.
4. **This repository's files.**
   - `REQUIREMENTS.md` decides numbers, gates and exceptions.
   - This `AGENTS.md` decides rules.
   - An `AGENTS.md` in a subdirectory decides for the files under it.
5. **Shared skills, templates and tool defaults.**

When the rules agree but the goals pull apart, weigh them in this order:

1. correctness and security of money and login;
2. integrity of stored data;
3. not breaking existing users and clients;
4. the agreed schedule;
5. the smallest change (no over-design);
6. consistency;
7. polish.

A higher goal never justifies more than the smallest change that meets it. When this order does not
settle a conflict, write it into the open-questions list and ask the owner.

## Where the rules live

| File | Read by | Holds |
| --- | --- | --- |
| `AGENTS.md` (this file) | every agent, loaded automatically | priority, hard limits, workflow, reporting, writing, commits |
| `.github/AGENTS.md` | agents changing CI files | CI tiers and gate rules |
| `web/AGENTS.md` | agents changing frontend files (loaded for files under `web/`) | layout, type, loading, i18n, accessibility |
| `crates/AGENTS.md` | agents changing backend files (loaded for files under `crates/`) | security, data, operations |
| `REVIEW.md` | reviewers; a review brief names it | how to review and report |
| `REQUIREMENTS.md` | everyone | numbers, gates, exceptions, deferred options, open questions |

Each rule is written in one file only.
A rule a tool can check belongs in a gate (lint, test, hook); prose is for rules that need judgment.

## Scope

- Apply these rules to the project as it is. Not every project or change involves every item.
- Skip sections the project does not have (frontend, backend, i18n, theming, admin panel).
- Do not add features, abstractions or tests only to satisfy this checklist.
- An item marked "(if present)" applies only when that feature already exists.
- These files hold rules only. The project's numbers, gates and exceptions (test time limits, sizes,
  widths, breakpoints, coverage) live in its own `REQUIREMENTS.md`, which wins on any number.
- A rule that needs its own gate, apart from the review of its section, carries a tag
  `[rule: <id>]`; the requirements check fails until it has a row named `path#id` in
  `REQUIREMENTS.md`, "Enforcement". Tag only a rule whose failure loses an account, a secret or
  data, and do not invent metrics for judgment rules.

---

## General (every change)

### Change risk

Class each change by what it can break, not by the files it touches. The class sets how much proof
the change needs; it never relaxes a hard limit. When unsure between two classes, take the higher.

| Class | Examples | Plan | Proof | Review |
| --- | --- | --- | --- | --- |
| Low | copy, docs, styling, a test-only change | the commit message | the checks of the touched area | the author's own diff read and the automated checks |
| Medium | a feature, an API change, a refactor | a short plan in the pull request | tests for the changed behaviour; impact checked | one diff review in a fresh context |
| High | money, login, security, a data migration, a deletion | a written plan, reviewed before coding | a test that fails without the fix; every behaviour hunk ablated; integration tests against the dependencies the change touches (a real database, an upstream's sandbox, never a live upstream) | an independent review by a different model family |

### Workflow

- Plan before coding, as far as the change's risk class asks: scope, impact, and how the change
  will be verified.
- Do not stop between milestones to wait on something that can run in the background.
- No over-engineering, no over-fixing, no cutting corners.
- A change must be a real improvement, not added complexity or a scattered patch.
- Fix a low-severity finding before release only when all three hold:
  - it is reachable;
  - it does concrete harm: money lands wrong, a card number or secret reaches storage or logs, or
    production is one mistyped command from a mistake;
  - the fix is small.
- Record every option or finding you set aside in the project's deferred-options list, with its
  source and the reason it waits. The owner decides from that list after release.

### When unsure

- Fix a verified defect inside the agreed scope, then report it.
- Do not decide or change anything when you cannot tell whether it is intentional, or when the
  change goes beyond the agreed scope.
- Write the question into an open-questions list and keep working on other unblocked tasks.
- Ask the owner all open questions together at a good stopping point. Ask at once only when the
  question blocks all remaining work.
- Check reachability before asking about or coding an edge case. A state that cannot occur gets no
  design.

### Reporting

- Report what was actually run: the commands, the tests and their results.
- Say clearly what was not verified. Never claim "tested" without running it.
- A check that reported nothing is evidence only when it saw real content. A sweep of pages that all
  rendered an error boundary proves nothing.

### Code quality

- Readability first; keep the logic clear.
- No spaghetti code.
- Remove useless comments, duplicate code and leftovers (dead code, debug output, temp files).
- Use the current idioms of the language and framework, but nothing new for its own sake. Adopt a
  newer library or language feature when it removes code or a known defect; say which in the pull
  request.
- When rewriting, match the existing code style, unless the owner has set a different convention.

### Impact and edge cases

- Check how the change affects other areas.
- Do not "fix" an intentional design choice as a bug (see "When unsure").
- Handle edge cases reasonably; no excessive defensive code.

### Secrets

- Never write keys, tokens or credentials to logs.
- Never store them in client-side storage (localStorage, sessionStorage, IndexedDB, or cookies
  without the right flags).
- Never hardcode them in source.

### Performance baseline

- Measure a baseline before a rewrite or an optimization.
- After the change, results must not be worse than the baseline.
- Measure both sides on the same host with its load recorded. Repeat a run before calling a
  difference real.

### Tests (if present)

- Keep tests lean and focused on critical paths.
- A bug fix comes with a test that fails without the fix. When a fix cannot have a test (build
  config, docs, a one-off script), say so in the report and give the command that verified it.
- A new check must be shown to fail on a planted violation before it counts.
- Ablation applies to a fix with two or more behaviour hunks. Remove each hunk once; a test must
  turn red.
  - In a High-risk change, ablate every hunk.
  - Elsewhere, ablate every hunk, or list in the report the hunks that were not ablated.
  - A one-hunk fix needs no extra step: the test that fails without the fix already shows it.
- The project sets a test time limit and measures against it. It also states which tier runs where
  and when (pre-commit, merge, release). Both are in `REQUIREMENTS.md`, "Commands".

### Merging

- A change merges once the gate is green and the review its risk class asks for has passed.
- A visible UI change also waits until the owner has seen screenshots or a preview.
- A release never merges on these rules alone; it waits for the owner.

### Documentation

- When behaviour changes, update the README, architecture and runbook text that describes it in
  the same change.

### Writing (docs, comments, reports, PR and issue text)

- **Order.**
  - Lead with the result, failure, decision or required action.
  - One action, result, reason or constraint per sentence.
  - Name the actor and the object; one term per concept.
- **Evidence.**
  - Verify every command, path, option, version, default and performance claim before writing it.
  - Label what is unverified. Never turn an expectation into a fact.
- **Style.**
  - Delete introductions, recaps, repeated conclusions, filler and promotional adjectives (robust,
    seamless, leverage, crucial, enhance).
  - No Emoji decoration, no bold wall, no em-dash rhythm.
  - Headings are sequential and name a task or subject.
- **Document type.** One reader goal per page, in one type: tutorial, how-to, reference or
  explanation.
- **README.** It opens with what the project is, how to install it, and the shortest successful
  use.
- **Comments.**
  - Comments state a contract or a reason the code cannot express: an invariant, a security
    boundary, a compatibility constraint, a workaround.
  - Delete comments that restate the code.
  - Follow the repository's comment language.
- **Chinese text.**
  - One script per document; never mix Simplified and Traditional.
  - Put a space between CJK and Latin letters or digits.
  - Use the established technical term.
  - Use the reader's locale vocabulary (Taiwan wording for zh-TW).
- **Interface copy.**
  - Labels name the action.
  - An error says what happened and what to do next.
  - Members never see upstream detail.

### Commits and pull requests

- **Scope.** One logical change per commit. Formatting and import reordering go in their own
  commit.
- **Subject.**
  - Short and imperative, at most 72 characters, naming the behaviour that changed.
  - In the repository's form, for example `area: subject`.
  - In English unless the repository uses another language.
- **Body.**
  - Why: the prior failure, why the new behaviour is correct, the compatibility consequence, and
    the issue id.
  - No work diary.
  - No routine test report: a green gate is a precondition, not a result.
- **Signing and mechanics.**
  - Every commit is signed. Never turn signing off to get past a stuck agent; fix the agent.
  - Write the message to a file and use `git commit -F`; use `--body-file` for PRs, never inline
    text with backticks.
  - Do not override the repository or global git config on the command line.
- **No AI attribution anywhere:**
  - no `Co-Authored-By` for a model;
  - no "generated with";
  - no tool or model names;
  - no session links.

  The author is the maintainer.
- **Fixups.** Squash `fixup!` and `squash!` commits before pushing.
- **Pull request description.** In this order, when each applies:
  1. the risk class and what in the diff puts it there;
  2. what fails or is needed;
  3. why;
  4. what changes and why that is correct;
  5. how the risky path was verified, with the exact commands;
  6. what remains unverified or limited.

  Leave out empty template sections and social closings.

---

## Both sides

### Frontend–backend mapping (when both exist)

- Every frontend call hits an existing backend endpoint.
- Every backend endpoint the frontend does not use has a written reason, such as an OAuth callback
  or use by a CLI or another client. Remove or add UI only for endpoints with no valid reason.
- Error codes and statuses map correctly to frontend notification levels.
- The browser talks only to the project's own origin. Third-party calls go through the backend.
