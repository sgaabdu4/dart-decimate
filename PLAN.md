# Avoid duplicate release setup and clean test fixtures

Status: Complete

## Outcome + scope

Run the existing Cargo/npm release-parity script directly so pnpm does not install root dependencies and compile the package before parity begins. Clean the temporary directories owned by the existing postinstall tests. Release these remaining corrections as 0.0.58 while preserving the independently merged analyzer, staged installer, tarball-path repair, and Hard Eng scaffold.

## Repository context

The Release workflow owns the parity invocation; `tests/ci_version_gate.rs` checks that command. `tests/js/postinstall.test.mjs` owns the existing installer failure cases. Established version owners are Cargo.toml, Cargo.lock, package.json, README.md, and docs/ci.md. Main's production installer validates both staged binaries before changing the real cache. No production installer or analyzer change is needed in this correction.

## Decisions + authorization

Blockers: None
Handoff: Ready for ship
Authority: The user authorized repository cleanup, pnpm migration, scaffold adoption, merge, and resulting native package release. Preserve concurrent upstream work and progress normally from main's 0.0.57 to 0.0.58. Keep every existing quality, coverage, release, and npm-consumer assertion. Add no dependency, test framework, or alternate runner.

## Acceptance + steps

- [x] Invoke the unchanged native parity script directly with its existing environment and all pack, Cargo, and npm assertions.
- [x] Update the existing workflow assertion to require that direct invocation.
- [x] Clean both existing postinstall temporary-directory owners through native test teardown, preserving all six installer assertions.
- [x] Preserve the incoming production staging, analyzer regressions, and packed-filename resolution; adopt the latest verified Hard Eng release through supported setup.
- [x] Synchronize established version owners at 0.0.58 and pass the existing version and unpublished-release guards.
- [x] Pass the required native check on the actual combined revision and record its evidence.

## Baseline + execution

Result: Passed
Evidence: The combined 0.0.58 revision passed all 30 native gates in 322.800 seconds on released Hard Eng 8693091. The preceding 0.0.57 revision passed all 30 native gates in 355.624 seconds, including 1,000 Rust tests, 37 JavaScript tests, the existing performance check, strict scanners, and actual npm CLI/MCP installations. That proof predates PR #128 and does not verify this combined revision. Supported fresh setup confirmed Hard Eng 96d5f9cbf00e0441e6225823f00d06990fa61068 without tracked changes.
Upstream: PR #127 delivered 0.0.56 with four platform assets and an npm attestation. PR #128 then advanced main to 0.0.57 with analyzer regressions and staged installer extraction. This branch integrates both changes without modifying their production or regression-test owners. Their staging implementation removes the need for additional fixture-loading machinery; tests continue to execute the actual postinstall script normally.
Scaffold: Supported fresh setup adopted released Hard Eng 8693091edc4071aefe7530d39a6337f99579876a in 53.899 seconds, verifying and committing only its three managed-file changes. The updater performed its scaffold verification; product verification remains the native full check below.
Repair: The previous failed release spent 90 seconds installing root dependencies and running postinstall before parity began. Direct Node execution retains the identical script, environment, and assertions, with no matching pre/post parity lifecycle scripts to preserve. Existing fixture directories now register native `node:test` cleanup at their creation sites.
Prior release gap: PR #125's Release run 36447841470 failed before tag creation and publication because the packed tarball's absolute path was duplicated. Main already contains the verified path-resolution repair. Version 0.0.55 was not published; this correction creates no competing historical tag.

## Risks + recovery

Keep the existing source/tag, published-version, provenance, and Cargo/npm parity guards. Preserve real npm/npx consumer compatibility fixtures and the unchanged production coverage threshold. Native pnpm publication retains its existing OIDC permissions; packaging or local installation does not prove hosted publication. Do not overwrite another agent's branch or a published version. Cleanup applies only to the temporary directories created by these tests, never shared stores or real package caches.

## ux_reference

N/A — this correction affects release execution and test cleanup, not a user interface.

## Verification

Result: Passed
Evidence: All 30 native gates pass on the combined 0.0.58 revision in 322.800 seconds: 1,003 Rust tests, 37 JavaScript tests, the existing performance check, strict scanners, release guards, and real npm compatibility journeys. The focused native JavaScript coverage command passes all 37 existing tests in 7.08 seconds. Hard Eng's actual report readers verify 822/944 production lines covered (87.08%; required 70%), including the real postinstall script. Checkout-cache hashes remain unchanged; no owned postinstall fixture or staging directory remains. Biome, strict TypeScript, edition-2024 Rust formatting, version synchronization, the 0.0.57-to-0.0.58 bump guard, unpublished-version check, migration check, and diff whitespace checks pass. Incoming production files match main exactly; scaffold changes come solely from the released installer. The full check confirms the same 822/944 line coverage (87.08%; required 70%).
Prior Cargo failure: An earlier full run passed Clippy but Cargo tests exited 101 before execution while writing `target/debug/.fingerprint/clap_builder-860318aa42d1d94c/invoked.timestamp` because its directory was absent. Clippy and tests are serial gates, target directories were writable, and bounded cleanup-owner tracing found no command explaining the missing directory. That failed run is not accepted proof. The following 0.0.57 and current 0.0.58 full checks passed without cache clearing or a workaround; the original cause remains unclassified. Stop and diagnose directly if it recurs.
E2E: Passed — existing real npm tarballs install from source and pass CLI help and MCP initialization on 0.0.58, including protocol 2025-11-25 and server version 0.0.58. CLI and MCP installation gates pass in 84.006 and 69.421 seconds; prebuilt installation journeys also pass. Verify the published binary separately after hosted publication.
Delivery target: Merge
Delivery: Pending — require normal native pre-push, exact-head PR checks, guarded merge, merged-commit checks, and the native Release workflow. Verify its tag, four platform assets, npm attestation, and isolated published-package binary. Hosted OIDC and publication remain distinct from local proof.
