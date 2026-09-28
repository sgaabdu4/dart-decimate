# Avoid duplicate release setup and clean test fixtures

Status: Draft

## Outcome + scope

Run the existing Cargo/npm release-parity script directly so pnpm does not install root dependencies and compile the package before parity begins. Clean the temporary directories owned by the existing postinstall tests. Release these remaining corrections as 0.0.58 while preserving the independently merged analyzer, staged installer, tarball-path repair, and Hard Eng scaffold.

## Repository context

The Release workflow owns the parity invocation; `tests/ci_version_gate.rs` checks that command. `tests/js/postinstall.test.mjs` owns the existing installer failure cases. Established version owners are Cargo.toml, Cargo.lock, package.json, README.md, and docs/ci.md. Main's production installer validates both staged binaries before changing the real cache. No production installer or analyzer change is needed in this correction.

## Decisions + authorization

Blockers: Current combined revision's required native verification is pending.
Handoff: Clarification
Authority: The user authorized repository cleanup, pnpm migration, scaffold adoption, merge, and resulting native package release. Preserve concurrent upstream work and progress normally from main's 0.0.57 to 0.0.58. Keep every existing quality, coverage, release, and npm-consumer assertion. Add no dependency, test framework, or alternate runner.

## Acceptance + steps

- [x] Invoke the unchanged native parity script directly with its existing environment and all pack, Cargo, and npm assertions.
- [x] Update the existing workflow assertion to require that direct invocation.
- [x] Clean both existing postinstall temporary-directory owners through native test teardown, preserving all six installer assertions.
- [x] Preserve the incoming production staging, analyzer regressions, packed-filename resolution, and installed Hard Eng revision unchanged.
- [x] Synchronize established version owners at 0.0.58 and pass the existing version and unpublished-release guards.
- [ ] Pass the required native check on the actual combined revision, then complete plan bookkeeping and normal pre-push delivery.

## Baseline + execution

Result: Pending
Evidence: The preceding 0.0.57 revision passed all 30 native gates in 355.624 seconds, including 1,000 Rust tests, 37 JavaScript tests, the existing performance check, strict scanners, and actual npm CLI/MCP installations. That proof predates PR #128 and does not verify this combined revision. Supported fresh setup confirmed Hard Eng 96d5f9cbf00e0441e6225823f00d06990fa61068 without tracked changes.
Upstream: PR #127 delivered 0.0.56 with four platform assets and an npm attestation. PR #128 then advanced main to 0.0.57 with analyzer regressions and staged installer extraction. This branch integrates both changes without modifying their production or regression-test owners. Their staging implementation removes the need for additional fixture-loading machinery; tests continue to execute the actual postinstall script normally.
Repair: The previous failed release spent 90 seconds installing root dependencies and running postinstall before parity began. Direct Node execution retains the identical script, environment, and assertions, with no matching pre/post parity lifecycle scripts to preserve. Existing fixture directories now register native `node:test` cleanup at their creation sites.
Prior release gap: PR #125's Release run 36447841470 failed before tag creation and publication because the packed tarball's absolute path was duplicated. Main already contains the verified path-resolution repair. Version 0.0.55 was not published; this correction creates no competing historical tag.

## Risks + recovery

Keep the existing source/tag, published-version, provenance, and Cargo/npm parity guards. Preserve real npm/npx consumer compatibility fixtures and the unchanged production coverage threshold. Native pnpm publication retains its existing OIDC permissions; packaging or local installation does not prove hosted publication. Do not overwrite another agent's branch or a published version. Cleanup applies only to the temporary directories created by these tests, never shared stores or real package caches.

## ux_reference

N/A — this correction affects release execution and test cleanup, not a user interface.

## Verification

Result: Pending
Evidence: The focused native JavaScript coverage command passes all 37 existing tests in 7.08 seconds. Hard Eng's actual report readers verify 822/944 production lines covered (87.08%; required 70%), including the real postinstall script. Checkout-cache hashes remain unchanged; no owned postinstall fixture or staging directory remains. Biome, strict TypeScript, edition-2024 Rust formatting, version synchronization, the 0.0.57-to-0.0.58 bump guard, unpublished-version check, migration check, and diff whitespace checks pass. Incoming production and scaffold files match main exactly. The full current revision's check is pending.
Prior Cargo failure: An earlier full run passed Clippy but Cargo tests exited 101 before execution while writing `target/debug/.fingerprint/clap_builder-860318aa42d1d94c/invoked.timestamp` because its directory was absent. Clippy and tests are serial gates, target directories were writable, and bounded cleanup-owner tracing found no command explaining the missing directory. That failed run is not accepted proof. The following 0.0.57 full check passed without cache clearing or a workaround; the original cause remains unclassified. Stop and diagnose directly if it recurs.
E2E: Required — preserve the existing real npm tarball CLI and MCP installation journeys on this revision; verify the published 0.0.58 binary after hosted publication.
Delivery target: Merge
Delivery: Pending — require normal native pre-push, exact-head PR checks, guarded merge, merged-commit checks, and the native Release workflow. Verify its tag, four platform assets, npm attestation, and isolated published-package binary. Hosted OIDC and publication remain distinct from local proof.
