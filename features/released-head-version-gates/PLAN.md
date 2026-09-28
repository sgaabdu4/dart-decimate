# Accept the released commit in the version gates

Status: Draft

## Outcome + scope

`version-bump` and `release-version` pass on a checkout whose clean HEAD carries the release tag for its version, the rule the Release workflow already applies, so the Hard Eng gate passes in the persistent checkout after it fast-forwards to a released main. Unbumped or already-published versions on any other commit still fail. Non-goals: Release workflow, tag creation and publishing.

## Repository context

Owners: `scripts/check-pr-version-bump.mjs`, `scripts/check-release-version.mjs`, `tests/js/release-guards.test.mjs`. Evidence: `.github/workflows/release.yml` sets `DART_DECIMATE_ALLOW_EXISTING_VERSION=1` when the version is published and its tag points at HEAD, and passes `BASE_SHA`; the local gate passes neither. On main at `2c4c42c` (tag `v0.0.57`), `node scripts/check-pr-version-bump.mjs` exits 1: `version must be bumped: 0.0.57 -> 0.0.57`. The session stop check in the persistent checkout failed `version-bump` and `release-version` after fast-forwarding to released main.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. On 2026-09-28 the owner asked to fix the failing stop check and follow Hard Eng exactly; AGENTS.md requires repairing reported gate failures as their own change.

## Acceptance + steps

- [ ] Version bump accepts an unchanged version when a clean HEAD carries its release tag → JS test, exit 0.
- [ ] Version bump still rejects tracked changes on top of the tagged commit → JS test, exit 1.
- [ ] Release check accepts a published version released from a clean HEAD → JS test, exit 0.
- [ ] Release check rejects a published version whose tag points at another commit → JS test, exit 1.
- [ ] Existing bump and registry cases hold → `node --test tests/js/release-guards.test.mjs` passes.
- [ ] Persistent checkout at a released main passes both gates → `node scripts/check-pr-version-bump.mjs` and `node scripts/check-release-version.mjs` exit 0 there.
- [ ] Full gate passes → `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete`.

## Baseline + execution

Result: Pending
Evidence: Pending — Draft check on `2c4c42c`.
Execution: One builder: failing JS tests first, then one shared release-tag check used by both scripts, version 0.0.58.

## Risks + recovery

An over-broad rule would let an unbumped or republished version through. Only an exact tag-to-HEAD match on a clean tracked tree passes, the same condition the Release workflow uses to allow an existing version. Recovery: revert the commit.

## ux_reference

N/A — release tooling only; no rendered interface.

## Verification

Result: Pending
Evidence: Pending — acceptance commands above.
E2E: Required — run both gate scripts in the persistent checkout at the released main and the full Hard Eng gate there.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.58.
