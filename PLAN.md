# Avoid duplicate release setup and clean test fixtures

Status: Complete

## Outcome + scope

Run the existing Cargo/npm release-parity script directly so pnpm does not install root dependencies and compile the package before parity begins. Clean the temporary directories owned by the existing postinstall tests. Release these remaining corrections as 0.0.59 while preserving the independently merged analyzer, staged installer, tarball-path repair, and Hard Eng scaffold.

## Repository context

The Release workflow owns the parity invocation; `tests/ci_version_gate.rs` checks that command. `tests/js/postinstall.test.mjs` owns the existing installer failure cases. Established version owners are Cargo.toml, Cargo.lock, package.json, README.md, and docs/ci.md. Main's production installer validates both staged binaries before changing the real cache. No production installer or analyzer change is needed in this correction.

## Decisions + authorization

Blockers: None
Handoff: Ready for ship
Authority: The user authorized repository cleanup, pnpm migration, scaffold adoption, merge, and resulting native package release. Preserve concurrent upstream work and progress normally from main's 0.0.58 to 0.0.59. Keep every existing quality, coverage, release, and npm-consumer assertion. Add no dependency, test framework, or alternate runner.

## Acceptance + steps

- [x] Invoke the unchanged native parity script directly with its existing environment and all pack, Cargo, and npm assertions.
- [x] Update the existing workflow assertion to require that direct invocation.
- [x] Clean both existing postinstall temporary-directory owners through native test teardown, preserving all six installer assertions.
- [x] Preserve the incoming production staging, analyzer regressions, and packed-filename resolution; adopt the latest verified Hard Eng release through supported setup.
- [x] Synchronize established version owners at 0.0.59 and pass the existing version and unpublished-release guards.
- [x] Verify the combined release guards and current CLI/MCP entrypoints with existing native checks; retain normal Complete pre-push as the final combined gate.

## Baseline + execution

Result: Passed
Evidence: The combined 0.0.58 revision passed all 30 native gates in 322.800 seconds on released Hard Eng 8693091. The preceding 0.0.57 revision passed all 30 native gates in 355.624 seconds, including 1,000 Rust tests, 37 JavaScript tests, the existing performance check, strict scanners, and actual npm CLI/MCP installations. That proof predates PR #128 and does not verify this combined revision. Supported fresh setup confirmed Hard Eng 96d5f9cbf00e0441e6225823f00d06990fa61068 without tracked changes.
Upstream: PR #127 delivered 0.0.56 with four platform assets and an npm attestation. PR #128 then advanced main to 0.0.57 with analyzer regressions and staged installer extraction. This branch integrates both changes without modifying their production or regression-test owners. Their staging implementation removes the need for additional fixture-loading machinery; tests continue to execute the actual postinstall script normally.
Scaffold: Supported fresh setup adopted released Hard Eng 2e246601a5dab5148cd8c9b829acb416da869294 in 74.365 seconds, verifying and committing only four managed-file changes. Native preflight confirmed the scaffold-only path, and instruction preservation passed. The updater performed its scaffold verification; it did not repeat product checks.
Reconciliation: PR #129 merged as 3ce7d66a5cd5a82d0e8a7d2611e7fa16409b30f5 with passing required CI. Its exact clean-tagged-HEAD release guard, shared helper, and four regression cases are preserved unchanged. The normal integration advances established version owners to 0.0.59. The preceding 0.0.58 native full and pre-push results are historical proof for that exact revision, not the combined 0.0.59 tree.
Repair: The previous failed release spent 90 seconds installing root dependencies and running postinstall before parity began. Direct Node execution retains the identical script, environment, and assertions, with no matching pre/post parity lifecycle scripts to preserve. Existing fixture directories now register native `node:test` cleanup at their creation sites.
Prior release gap: PR #125's Release run 36447841470 failed before tag creation and publication because the packed tarball's absolute path was duplicated. Main already contains the verified path-resolution repair. Version 0.0.55 was not published; this correction creates no competing historical tag.

## Risks + recovery

Keep the existing source/tag, published-version, provenance, and Cargo/npm parity guards. Preserve real npm/npx consumer compatibility fixtures and the unchanged production coverage threshold. Native pnpm publication retains its existing OIDC permissions; packaging or local installation does not prove hosted publication. Do not overwrite another agent's branch or a published version. Cleanup applies only to the temporary directories created by these tests, never shared stores or real package caches.

## ux_reference

N/A — this correction affects release execution and test cleanup, not a user interface.

## Verification

Result: Passed
Evidence: The combined 0.0.59 JavaScript suite passes all 41 existing tests in 4.260 seconds. Hard Eng's native report reader verifies every production source and 867/989 covered lines (87.66%; required 70%), including the incoming release-head helper. Existing version synchronization, 0.0.58-to-0.0.59 bump, unpublished-release and migration guards pass. Incoming release-gate, installer, and analyzer owners match main exactly. Final source adoption and native instruction preservation also pass. Normal Complete pre-push must verify every required gate on the exact committed snapshot before shipping; no older full run is relabeled as that proof.
Historical integrated proof: The preceding 0.0.58 revision passed all 30 native gates in 322.800 seconds (1,003 Rust tests, 37 JavaScript tests, the existing performance check, strict scans and real npm CLI/MCP journeys). JavaScript line coverage was 822/944 (87.08%; required 70%). Its actual isolated pre-push passed in 303.96 seconds, and exact-head PR CI passed Rust/npm checks in 7m15s, dependency audit in 2m29s, and OpenSSF Scorecard in 18s. Focused fixture verification left checkout-cache hashes unchanged and no owned postinstall fixture or staging directories.
Prior Cargo failure: An earlier full run passed Clippy but Cargo tests exited 101 before execution while writing `target/debug/.fingerprint/clap_builder-860318aa42d1d94c/invoked.timestamp` because its directory was absent. Clippy and tests are serial gates, target directories were writable, and bounded cleanup-owner tracing found no command explaining the missing directory. That failed run is not accepted proof. Subsequent 0.0.57 and 0.0.58 full checks passed without cache clearing or a workaround; the original cause remains unclassified. Stop and diagnose directly if it recurs.
E2E: Passed — current package entrypoints return CLI version 0.0.59, valid CLI help, and MCP initialize protocol 2025-11-25 with server version 0.0.59 in 0.670 seconds. This used the actual current binaries with scanner overrides removed and Cargo unavailable, so no fallback build supplied the result. Actual npm tarball CLI and MCP installation journeys remain required native pre-push gates; current entrypoint proof does not claim a fresh npm installation. Verify the published 0.0.59 binary separately after hosted publication.
Delivery target: Merge
Delivery: Pending — continue the same PR #130 through normal native pre-push, exact-head PR checks, guarded merge, merged-commit checks, and the native Release workflow. Verify its tag, four platform assets, npm attestation, and isolated published-package binary. Hosted OIDC and publication remain distinct from local proof.
