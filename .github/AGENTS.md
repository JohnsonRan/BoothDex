# .github/AGENTS.md: CI rules

Rules for every change to CI configuration and gate scripts. The root `AGENTS.md` applies as well;
tier time limits and commands are in `REQUIREMENTS.md`. Tools load this file only for files under
`.github/`; name it in the brief when a change touches a gate script under `scripts/`.

## CI

### Tiers, jobs and gates

Both mistakes are wrong: over-designing CI, and putting everything into one CI run.

- **Split by cost and timing.** Each tier has its own time limit in `REQUIREMENTS.md`.
  - Pre-commit runs the read-only fast checks for the changed files, in seconds.
  - The merge gate runs the checks that guard the changed paths.
  - A scheduled run on main covers the heavy suites: full race, end-to-end, self-tests,
    performance.
  - A gate's self-test plants one violation and expects red; that is what shows the gate is
    wired. It runs in the merge gate when that gate's script changes, and in the release tier
    for every gate command in `REQUIREMENTS.md`, "Enforcement".
  - A release adds release-only checks: a reproducible build, migrations on a copy of production
    data, a deploy rehearsal.
- **One job per concern, grouped by the area it guards.** Do not build one giant job, where one red
  step hides the rest and every rerun costs everything. Do not build one job per tiny check either.
- **No CI nobody needs.**
  - No OS or version matrix the project does not support.
  - No cache layer that saves seconds.
  - No duplicate runs of the same suite in two tiers without a reason.
  - Required checks are only the ones that decide a merge.
- **Know what the merge gate skipped.** A suite it skips runs in full on the final commit or in
  the scheduled run.
- **Same script in CI and locally.** CI calls the repository's own script. Use an existing action
  pinned by SHA instead of copying its code into the repository.
- **Checks that can turn red.**
  - A rule without a check that turns red is a preference.
  - Every check fails on a planted violation, and fails when it scans nothing.
  - Exit code 1 means a violation was found; exit code 2 means the check could not run.
- **Flaky tests.** Fix a flaky test, or quarantine it with an owner and a date. Never retry it
  silently. Quarantine is the only allowed way to take a check out of a gate; it is not "switching
  the gate off" under the hard limits.
