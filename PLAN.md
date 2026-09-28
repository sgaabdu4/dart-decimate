# Release parity repair and final scaffold update

Status: Complete

## Outcome + scope

Avoid repeating root installation before native Cargo/npm release parity and isolate postinstall tests without losing production coverage. Preserve the pnpm tarball path repair, analyzer and stale-cache fixes, released Hard Eng scaffold, Claude/Codex wiring, and all existing release assertions now on main.

## Repository context

Owners: `package.json`, `scripts/`, and `hard-eng.gates.json` own checks; `src/ci_template.rs` and `src/hooks/templates.rs` generate commands; `tests/` owns native regression and package compatibility contracts. Existing CI, Security, and Release workflows own PR verification, audits, and tagged publication. Unrelated feature work and the open dependency update remain outside this PR.

## Decisions + authorization

Blockers: None
Handoff: Ready for ship
Authority: The user authorized the combined repository cleanup, pnpm migration, scaffold update, merge, and resulting native package release. Continue with one corrective PR for the remaining parity invocation and fixture repairs at version 0.0.57, preserving independently merged work. Keep the existing quality and compatibility assertions; add no alternate runner or artificial tests.

## Acceptance + steps

- [x] Migrate supported npm/npx producers to pnpm while preserving npm registry metadata and actual npm/npx consumer tests.
- [x] Preserve published-version, source/tag, provenance, and Cargo/npm parity assertions for version 0.0.55.
- [x] Use native pnpm bootstrap in generated CI and local-only pnpm execution in agent hooks.
- [x] Audit the actual PR/push comparison and block native failure verdicts in generated agent hooks.
- [x] Retire obsolete no-mistakes configuration and unsupported agent/tool wiring through the released installer.
- [x] Preserve all native checks, benchmark, and coverage; review the final public diff.
- [x] Resolve pnpm's actual tarball filename without duplicating an absolute path; prove both path forms against a real packed archive.
- [x] Keep established version owners synchronized at 0.0.57 and invoke the same parity script directly without root installation.
- [x] Isolate the existing postinstall fixtures from real checkout binaries while preserving their failure assertions and cleaning only fixture-owned paths.
- [x] Preserve the current verified scaffold and pass the corrective revision's required checks.

## Baseline + execution

Result: Passed
Evidence: The reconciled 0.0.57 revision passed the complete native check in 355.624 seconds against current main, including 1,000 Rust tests, 37 JavaScript tests, the existing performance check, strict scanners, and real npm CLI/MCP installations. Supported fresh setup confirmed installed Hard Eng 96d5f9cbf00e0441e6225823f00d06990fa61068 is current, without tracked changes.
Previous failed baseline: PR #125 merged as 0150e1e98a82a29e50d3383716cddd4056f9e101. Required checks and all four platform builds passed, but Release run 36447841470 failed Cargo/npm parity with an npm ENOENT caused by the duplicated tarball path. Tag creation, GitHub release, and npm/OIDC publication were not reached; tag v0.0.55 and npm version 0.0.55 were both absent when inspected.
Prior baseline: The committed 0.0.55 candidate passed all 30 native gates against published Hard Eng d2085f745de39214aaaf6b34192378c9d1094a3d before installation. The initial baseline had passed native tests and scans but refused installation because 0.0.54 was unchanged and already published; its target remained unchanged.
Additional baseline evidence: A native two-commit fixture proved that comparing HEAD with itself skips a newly introduced finding. A real cyclic-import audit returned exit 1 and a failure verdict, but the existing generated agent hook returned 0. Bare pnpm exec also installed dependencies and ran root postinstall before executing its command.
Execution: Bumped the authorized release version, adapted existing native producers and assertions, and committed the reviewed application changes before the successful updater run. Its isolated candidate starts from committed HEAD; an uncommitted retry was stopped after identifying its stale 0.0.54 snapshot and is not verification of this change. The released installer then committed the verified scaffold update.
Repair: Preserve main's packed-filename resolution so an absolute result remains unchanged. Release now runs the existing Node script directly with identical environment and assertions; the failed run spent 90 seconds installing root dependencies and running postinstall before parity even began. There are no pre/post parity lifecycle scripts to preserve. Version 0.0.57 satisfies the retained requirement to increase above main's 0.0.56; no guard is relaxed.
Reconciliation: PR #127 merged while this repair was being verified. Its analyzer regressions, production stale-cache removal, tarball-path fix, three scoped plans, and verified Hard Eng 96d5f9 scaffold are preserved unchanged by merging current main. The source-marker conflict is resolved to that incoming installed revision. This correction adds no competing 0.0.56 tag or release.
Upstream delivery: PR #127's Release run 36468515282 subsequently passed; v0.0.56 points to 0f401e4b85c11f1cfb1af1e538019f6bdd184d79, has all four platform assets, and is published on npm with an attestation. This corrective PR progresses normally to 0.0.57.
Fixture repair: The empty-asset case ran the checkout's postinstall script and found its real compiled binaries, returning 0 instead of the required 127. Each existing fixture now runs the unchanged source through Node's native `vm.compileFunction`, retaining the production filename for exact coverage attribution and supplying a temporary package's `__dirname` and `createRequire` context. Its minimal package manifest uses the current package version. A copied-script attempt preserved assertions but omitted production coverage; that failed result is not accepted. The real checkout cache remains intact; native test cleanup removes only fixture-owned paths. Existing packed-install checks still exercise the normal Node loader independently.

## Risks + recovery

pnpm pack returns one metadata object; release parity must install that actual tarball with npm. Native pnpm publishing uses existing OIDC and provenance permissions; the already-verified detached release checkout needs only publication's `--no-git-checks`. Registry transport/authentication failures remain blocking. The existing rejection test uses a native HTTP 401 response to protect the owned fail-closed lookup outcome without waiting for pnpm's HTTP 500 backoff; production retry behavior is unchanged. Its temporary projects and caches now use native test cleanup. Retain existing tagged-source repair guards; never overwrite an unrelated release or treat dry-run packaging as hosted publication proof.

## ux_reference

N/A — this migration changes CLI installation, generated commands, and repository checks; it does not alter the rendered HTML report.

## Verification

Result: Passed
Evidence: All native gates pass on the reconciled 0.0.57 revision in 355.624 seconds. Rust tests pass 1,000 cases; JavaScript tests pass 37 cases with 818/935 production lines covered (87.49%; required 70%). The existing performance check, Clippy, strict types, formatting, security/dependency scans, release guards, and native npm compatibility journeys pass. Version synchronization, the current 0.0.56-to-0.0.57 bump guard, unpublished-version check, and migration check pass. The Release command invokes the identical parity script directly, preserving its environment and every assertion; it has no pre/post lifecycle scripts. Hosted source/asset parity and publication remain delivery requirements.
Path proof: A real pnpm pack reproduces the old absolute-filename error. The preserved path repair resolves both absolute and relative filenames, and npm installs the resulting archive with lifecycle scripts disabled; this bounded proof passed in 0.816 seconds.
Corrective check: The first full run passed Clippy but Cargo tests exited 101 while writing `target/debug/.fingerprint/clap_builder-860318aa42d1d94c/invoked.timestamp` because its directory was absent. Cargo tests had not executed. Clippy and tests are serial gates, target directories are writable, and the bounded repository cleanup trace found no command explaining that missing directory. The run was stopped after these findings and is not successful integrated proof. No build cache was cleared or gate weakened.
Fixture proof: The unchanged empty-asset assertion failed before the repair with real checkout binaries present. After reconciling current main, the native JavaScript coverage command passes all 37 existing tests in 7.198 seconds; the unchanged Hard Eng report reader confirms all expected production files, including `npm/scripts/postinstall.js`, and 818/935 covered lines (87.49%; minimum 70%). Native module-resolution proof reads the fixture's distinct package metadata. Checkout-cache hashes are unchanged and no postinstall-owned temporary paths remain. Biome, strict TypeScript checking, Rust formatting with the manifest's edition, and the diff whitespace check pass. The full integrated run subsequently passed without the earlier Cargo fingerprint failure. Its cause remains unclassified; no cache was cleared or workaround added.
Prior integrated proof: The original candidate passed 996 Rust tests, 37 JavaScript tests, and the existing performance check. Line coverage was 818/932 (87.77%); its final native pre-push passed in 187.79 seconds. PR and merged required checks passed. The existing generated-shell regression proves failure verdict exit 1 blocks with exit 2 while runtime errors remain allowed; real cycle and runtime-error probes agree. Those results do not establish the failed release's Cargo/npm parity or publication.
E2E: Passed — real 0.0.57 npm tarballs install from source and pass the existing CLI help and MCP initialize journeys, including MCP protocol 2025-11-25 and server version 0.0.57. Existing prebuilt-install journeys also pass. Hosted Cargo/source-asset parity and published-package proof remain part of the release delivery requirements below.
Delivery target: Merge
Delivery: Pending — require native pre-push, current PR and merged-commit checks, then verify the native GitHub release/tag and npm version produced by the exact merged revision. The retained release workflow must pass Cargo/npm install parity and hosted OIDC publication; local package proof does not replace those checks.
