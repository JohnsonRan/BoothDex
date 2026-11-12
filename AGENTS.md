# AGENTS.md: development rules

Every agent that plans, writes or reviews a change in this repository follows this file and the
files it names.

## Rule priority

When two rules disagree, the higher one wins. Within a level, the more specific rule wins over the
general one, and the owner's newer statement wins over an older one.

1. **Hard limits.** Nothing below overrides these:
   - no secret or card number in logs, in source, or in client storage (localStorage,
     sessionStorage, IndexedDB, or cookies without the right flags);
   - no production step without the owner's explicit go;
   - no test traffic to a real upstream unless the owner allows it;
   - no AI attribution (when an upstream project requires AI disclosure, stop and ask the owner;
     write no disclosure yourself);
   - no bypassing signing, review or a gate by switching it off (quarantining a flaky test with an
     owner and a date is the one allowed exception).
2. **The owner's explicit instruction for the current task.**
3. **The owner's recorded rulings.** A ruled behaviour is not a finding and is not asked again.
4. **This repository's files.** `REQUIREMENTS.md` decides numbers, gates and exceptions. The
   `AGENTS.md` files decide rules; one in a subdirectory decides for the files under it.
5. **Shared skills, templates and tool defaults.**

When the rules agree but the goals pull apart, weigh them in this order: correctness and security
of login; integrity of stored data; not breaking existing users and clients; the agreed schedule;
the smallest change; consistency; polish. A higher goal never justifies more than the smallest
change that meets it. When this order does not settle a conflict, ask the owner (see "Workflow").

## Where the rules live

| File | Read by | Holds |
| --- | --- | --- |
| `AGENTS.md` (this file) | every agent, loaded automatically | priority, hard limits, workflow, writing, commits |
| `.github/AGENTS.md` | agents changing CI files or gate scripts | CI tiers and gate rules |
| `web/AGENTS.md` | agents changing files under `web/` | layout, type, loading, i18n, accessibility |
| `crates/AGENTS.md` | agents changing files under `crates/` | security, data, operations |
| `REVIEW.md` | reviewers; a review brief names it | how to review and report |
| `REQUIREMENTS.md` | everyone | numbers, gates, exceptions, deferred options, open questions |

- Each rule is written in one file only. A rule a tool can check belongs in a gate (lint, test,
  check script); prose is for rules that need judgment.
- Apply the rules to the project as it is. An item marked "(if present)" applies only when that
  feature exists. Do not add features, abstractions or tests only to satisfy these files.
- A rule that needs its own gate, apart from the review of its section, carries a tag
  `[rule: <id>]`; the requirements check fails until it has a row named `path#id` in
  `REQUIREMENTS.md`, "Enforcement". Tag only a rule whose failure loses an account, a secret or
  data, and do not invent metrics for judgment rules.

## Every change

### Change risk

Class each change by what it can break, not by the files it touches. The class sets how much proof
the change needs; it never relaxes a hard limit. When unsure between two classes, take the higher.

| Class | Examples | Plan | Proof | Review |
| --- | --- | --- | --- | --- |
| Low | copy, docs, styling, a test-only change | the commit message | the checks of the touched area | the author's own diff read and the automated checks |
| Medium | a feature, an API change, a refactor | a short plan in the pull request | tests for the changed behaviour; impact checked | one diff review in a fresh context |
| High | login, security, a data migration, a deletion | a written plan, reviewed before coding | a test that fails without the fix; every behaviour hunk ablated; integration tests against the dependencies the change touches (a real database, an upstream's sandbox, never a live upstream) | an independent review by a different model family |

### Workflow

- Plan before coding, as far as the risk class asks: scope, impact, and how the change will be
  verified.
- No over-engineering, no over-fixing, no cutting corners. A change is a real improvement, not
  added complexity or a scattered patch.
- Check how the change affects other areas. Check reachability before handling an edge case: a
  state that cannot occur gets no design and no defensive code.
- Fix a verified defect inside the agreed scope, then report it. When you cannot tell whether
  something is intentional, or a change goes beyond the agreed scope, change nothing: write the
  question under "Open questions" in `REQUIREMENTS.md` and keep working on other tasks. Ask the
  owner all open questions together at a stopping point, or at once only when the question blocks
  all remaining work.
- Fix a low-severity finding before release only when it is reachable, the fix is small, and it
  does concrete harm: a card number or secret reaches storage or logs, or production is one
  mistyped command from a mistake.
- Record every option or finding you set aside under "Deferred options" in `REQUIREMENTS.md`, with
  its source and the reason it waits. The owner decides from that list after release.
- When behaviour changes, update the README, architecture and runbook text that describes it in
  the same change.

### Reporting

- Report what was actually run: the commands, the tests and their results.
- Say clearly what was not verified. Never claim "tested" without running it.
- A check that reported nothing is evidence only when it saw real content. A sweep of pages that all
  rendered an error boundary proves nothing.

### Code quality

- Readability first. No spaghetti code, duplicate code or leftovers (dead code, debug output, temp
  files).
- Use the current idioms of the language and framework, but nothing new for its own sake. Adopt a
  newer library or language feature when it removes code or a known defect; say which in the pull
  request.
- Match the existing code style, unless the owner has set a different convention.

### Performance baseline

- Measure a baseline before a rewrite or an optimization. After the change, results are not worse.
- Measure both sides on the same host with its load recorded. Repeat a run before calling a
  difference real.

### Tests (if present)

- Keep tests lean and focused on critical paths. Time limits and tiers are in `REQUIREMENTS.md`,
  "Commands".
- A bug fix comes with a test that fails without the fix. When a fix cannot have a test (build
  config, docs, a one-off script), say so in the report and give the command that verified it.
- A new check is shown to fail on a planted violation before it counts.
- In a fix with two or more behaviour hunks, remove each hunk once; a test must turn red. A
  High-risk change ablates every hunk; elsewhere, list the hunks that were not ablated.

### Merging

- A change merges once the gate is green and the review its risk class asks for has passed.
- A visible UI change also waits until the owner has seen screenshots or a preview.
- A release never merges on these rules alone; it waits for the owner.

### Writing (docs, comments, reports, PR and issue text)

- Lead with the result, failure, decision or required action. One action, result, reason or
  constraint per sentence. Name the actor and the object; one term per concept.
- Verify every command, path, option, version, default and performance claim before writing it.
  Label what is unverified; never turn an expectation into a fact.
- No introductions, recaps, repeated conclusions, filler or promotional adjectives (robust,
  seamless, leverage, crucial, enhance). No Emoji decoration, no bold wall, no em-dash rhythm.
  Headings are sequential and name a task or subject.
- One reader goal per page, in one type: tutorial, how-to, reference or explanation. A README
  opens with what the project is, how to install it, and the shortest successful use.
- Comments state a contract or a reason the code cannot express: an invariant, a security
  boundary, a compatibility constraint, a workaround. Delete comments that restate the code.
  Follow the repository's comment language.
- Chinese text: one script per document, never mixing Simplified and Traditional; a space between
  CJK and Latin letters or digits; the established technical term; the reader's locale vocabulary
  (Taiwan wording for zh-TW).
- Interface copy: labels name the action; an error says what happened and what to do next; members
  never see upstream detail.

### Commits and pull requests

- One logical change per commit. Formatting and import reordering go in their own commit.
- Subject: imperative, at most 72 characters, naming the behaviour that changed, in the form
  `area: subject`, in English.
- Body: why. The prior failure, why the new behaviour is correct, the compatibility consequence,
  and the issue id. No work diary and no routine test report.
- Every commit is signed. Never turn signing off to get past a stuck agent; fix the agent.
- Write the message to a file and use `git commit -F`, and `--body-file` for a pull request; never
  inline text with backticks. Do not override the repository or global git config on the command
  line.
- No AI attribution anywhere: no `Co-Authored-By` for a model, no "generated with", no tool or
  model names, no session links. The author is the maintainer.
- Squash `fixup!` and `squash!` commits before pushing.
- A pull request description holds, in this order and when each applies: the risk class and what
  in the diff puts it there; what fails or is needed, and why; what changes and why that is
  correct; how the risky path was verified, with the exact commands; what remains unverified or
  limited. Leave out empty template sections and social closings.

## Both sides

### Frontend–backend mapping (when both exist)

- Every frontend call hits an existing backend endpoint.
- Every backend endpoint the frontend does not use has a written reason, such as an OAuth callback
  or use by a CLI or another client. Remove or add UI only for endpoints with no valid reason.
- Error codes and statuses map correctly to frontend notification levels.
- The browser talks only to the project's own origin. Third-party calls go through the backend.
