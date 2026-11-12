# REVIEW.md: reviewing a change (frontend and backend)

Every agent that reviews a change in this repository follows this file. The rules the change must
meet are in `AGENTS.md`, `web/AGENTS.md`, `crates/AGENTS.md` and `REQUIREMENTS.md`; this file says
how to review against them and what a review report must contain. It does not repeat them.

## Rule priority

The rule priority in the root `AGENTS.md` applies. For a review it means: a finding against a hard
limit or a money or login invariant outranks every structure or style finding, and a style finding
never blocks a fix of a higher goal.

## Before you start

- Review read-only. Never edit, commit to, rebase or push the branch under review.
- Record three SHAs in the report: the target's base, the branch head, and their merge-base.
  Review the fixed `base...head` diff, not the two branch tips.
- Read, in order:
  1. the root `AGENTS.md`, the `AGENTS.md` of every directory the diff touches, and
     `REQUIREMENTS.md`;
  2. the plan or brief of this change;
  3. the owner's recorded rulings.
- A behaviour the owner has ruled on is not a finding.
- Do not read other reviews, bot comments or the PR discussion until your own pass is finished.
  Then re-check what they found and you missed, merge duplicates, and mark which findings came from
  them.
- Review in a fresh context with none of the writer's history; for a High-risk change (root
  `AGENTS.md`, "Change risk"), use a different model family from the writer. Put the model id from
  your own run's record in the report. When only one family is available, the owner reviews
  High-risk changes.

## Scope

- Check the risk class against what the diff touches (root `AGENTS.md`, "Change risk"). A class
  set too low is the first finding; review at the correct class.
- Review this change, not the repository. Pre-existing defects are out of scope.
- Exception: a change that makes an existing defect reachable for the first time. Report it, in a
  separate list.
- If the plan has a change map, compare it with the diff first:
  - every node marked as changed must appear in the diff;
  - every file the diff touches must be on the map.
  - A mismatch is the first finding.
- When the change patches a shape that no longer holds, the first finding is "rewrite this, do not
  patch it", naming the function. Signs of such a shape:
  - a third patch in the same place;
  - no invariant anyone can state;
  - callers that need a special case after the fix.

## Every finding has evidence

- A behaviour finding says the code does the wrong thing when it runs: a wrong result, money,
  login, data, a crash. Before it goes into the report, reproduce it yourself with one of:
  - a failing test;
  - a query on a scratch database;
  - a command and its output;
  - a reproduction in the running app.
- A structure finding says the code is shaped wrongly while it still behaves correctly: duplicate
  or dead code, logic in the wrong layer, file size, naming, unused keys. A precise reading is
  enough evidence for it:
  - `path:line` for every place the claim covers;
  - the search command and its output; for "nothing uses this", show that the search read real
    files;
  - for "this can be deleted", what breaks if the claim is wrong.
- A finding with reading-only evidence is Medium at most. To rate it higher, reproduce it.
- Exception: a behaviour that cannot be triggered safely even in a disposable environment, because
  it needs a real upstream or production-only state, or would destroy data others use. Its evidence
  is a complete static trace: `path:line` for every step from input to effect, with the condition
  at each branch. Mark it "static, not reproduced: <reason>". It keeps the severity the trace
  supports.
- A pass-bar item blocks through the pass bar, not through its severity.
- Report what you ran and what it showed.
- What you could neither reproduce nor show by a precise reading goes into a separate
  "Hypotheses" list, with the command that would settle it. A hypothesis never counts as a
  finding.
- Spend reproduction effort in proportion to the risk class and the severity. A suspected defect
  that would take longer to reproduce than the change took to write goes to "Hypotheses" with its
  command, unless it is High-risk.
- Check reachability first. A state that cannot occur with current data or users is not reported.
- Count carefully. Measure exactly the thing the sentence claims, not a wider set (for example,
  calls versus comment lines that mention the name).

## Severity proves itself

| Severity | Meaning | Required evidence |
| --- | --- | --- |
| Blocker | Money lost or created, an account taken over, a secret or card number exposed, data destroyed, or production unable to run. | The trigger path: input, state, what happens. Reproduced, or a complete static trace under the exception above. |
| High | The same classes, behind a precondition an attacker or ordinary use can meet. Or a core flow that fails for a whole class of users. | Trigger path and reproduction, or a complete static trace under the exception above. |
| Medium | A real defect with limited reach: a recoverable lockout, a wrong value an operator would notice, a security weakness that needs another failure first. | Reproduction, or a precise reading for a structure finding. |
| Low | Correct but fragile, misleading or untidy. | Reproduction or a precise reading. |

- A severity without a trigger path is downgraded until it has one.
- Do not report as a finding a change that removes a protection on purpose and within scope.
  Report it only when you judge the author missed a consequence, and name that consequence.

## Pass bar

These block a pass unless the author states why this case is different:

- A clearly simpler way would remove a whole block of complexity, and the change keeps it.
- A file moves above the repository's p90 size with no reason given for not splitting it.
- A special case is inserted into an existing path and makes it harder to read.
- A local problem is solved by scattering feature checks through shared code.
- A wrapper, conversion or optional layer is added that buys no clarity.
- A near-duplicate of an existing helper is written.
- A mechanism the platform or a dependency already provides is rewritten or wrapped.
- A script, service unit or CI workflow carries more lines than its peers in comparable projects,
  without a reason for each extra line.
- A new comment restates the code, or is denser than the code around it.
- Logic sits in the wrong layer.

Behaviour being correct is not a reason to pass; it only means nothing regressed.

## Checking against the rules

Go through `web/AGENTS.md` for frontend files and `crates/AGENTS.md` for backend files, rule by
rule. This section only says how to verify the rules a diff cannot show.

- **Layout, type and loading.**
  - Open the screenshots yourself: phone and desktop widths, every theme, real data.
  - Measure edges, gaps and title positions on the shot or in the browser; do not judge by eye.
  - A shot of an error boundary or an empty state does not count.
- **Copy and i18n.** Run the project's i18n check. Read every changed string in every locale.
- **Accessibility.** Tab through the changed flow. Measure contrast with a tool.
- **Money and state.**
  - Reproduce against a real database, with repeated and concurrent requests and out-of-order
    callbacks.
  - A mock-only test never proves a money path.
- **Authorization.** Call each changed endpoint as the wrong role and from the wrong surface.
  Check that every route without an authorization check is marked public.
- **Sensitive data.** Write a planted card number through each changed free-text path, then search
  every log of that run, the database's included.
- **Migrations.** Apply on a database that holds rows, apply again, and compare the number with
  main.
- **Performance.** When the diff changes code that `scripts/bench/run.sh` or `bun run check:size`
  (in `web/`) measures, the pull request carries both runs, before and after, from the same host
  with its load. A result worse than the baseline is a finding unless the owner accepted it.

Review-only checks that no development rule covers:

- **Local setup.** Flag a change that breaks other people's local run:
  - a renamed environment variable;
  - a new key file or path;
  - a changed port or container wiring;
  - a script that must run once before development can continue.
- **Feature-flag leaks.** Nothing behind a flag (a name, a route, a message) may be visible or
  callable while the flag is off.
- **Newly reachable defects.** List a pre-existing defect that this change makes reachable for the
  first time.

## CI, docs and commits

- **CI.**
  - Each check sits in the right tier: pre-commit, merge gate, scheduled run or release.
  - No new giant all-in-one job, and no one-job-per-trivial-check sprawl.
  - A new check is shown to fail on a planted violation, and fails when it scans nothing.
  - Every gate command in `REQUIREMENTS.md`, "Enforcement", is run by a tier in "Commands" (name
    where) and fails on a planted violation. The requirements check only shows that its files
    exist.
- **Docs.**
  - Behaviour changes update the README, architecture and runbook text in the same change.
  - Every command, path and number in the docs matches the code.
  - A Due date in `REQUIREMENTS.md`, "Not yet enforced", moved later carries the owner's approval,
    quoted in the commit body. So does a tagged rule (`path#id`) marked as not applying.
- **Commits.**
  - One logical change each; an imperative subject; a body that says why.
  - Signed.
  - No AI attribution, no work diary, no routine test report.
  - No `fixup!` or `squash!` subjects left.

## The writer's report

Check the pull request description, or the writer's report when there is no pull request, against
the root `AGENTS.md`, "Reporting" and "Waiting":

- **Reporting.** Rerun at least one command the report names and compare the result. A test or
  check claimed without its command and result is a finding, and so is an unverified part that is
  not labelled.
- **Waiting.** A job that ran across several agents or wake-ups has its STATE file current to its
  last milestone. A step stuck past the wait timeout in `REQUIREMENTS.md` is recorded with its
  state, not dropped or retried without a word.

## Report

```text
Verdict: PASS | FIX FIRST | BLOCK        (one line, first; rule below)
Model: <id from the run record>   Base / head / merge-base: <sha> <sha> <sha>
Covered: files and flows read; Not read: what and why
Findings, by severity, then by the order below:
  [severity] path:line, one-sentence defect
    scenario: input → state → wrong result
    evidence: command or test and what it showed; or path:line and the search output
    wanted: the smallest change that closes it
Made reachable by this change (pre-existing defects): ...
Hypotheses (no evidence yet): each with the command that would settle it
Over-design: against the pass bar
Deferred-options candidates: options set aside, with their reason
From other reviewers: which findings came from them
```

- Verdict rule:
  - BLOCK when there is any Blocker;
  - FIX FIRST when there is any High or Medium, or any pass-bar item;
  - PASS when only Lows remain; each Low is fixed or put on the deferred-options list by the low
    rule in the root `AGENTS.md`.
- Order findings by severity first. Within one severity, order them by structure:
  1. structural regression (harder to read, more coupled, more stateful);
  2. complexity that could be deleted outright;
  3. branch complexity added to an existing path;
  4. boundary and contract problems;
  5. file and module size;
  6. readability.
- Do not bury one large problem under ten small ones. Three high-confidence findings beat fifteen
  mixed ones.
- Keep the report short. It holds no restated diff and no list of checks that passed.
