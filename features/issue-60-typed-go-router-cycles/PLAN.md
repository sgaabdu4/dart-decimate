# Feature Brief: Issue 60 Typed Go Router Cycles

<!-- hard-eng-state:v1 -->
- state_version = 1
- plan_id = issue-60-typed-go-router-cycles-77e91fae
- lifecycle_status = green
- approval_status = approved
- approval_fingerprint = sha256:b67c26b9792e1e2b8166f84d095300e82f9c80c8fc6d03456bb379c5c004cc8b
- approval_provenance = ready-to-build
- green_artifact = sha256:256b322e1d47778bff6c368b34ff422648b4630f53964d34f09d235f77835524
- active_slice = none
- completed_slices = S-1,S-2
- next_action = Commit the corrected green artifact, push codex/issue-60-typed-go-router-cycles, resolve the Copilot thread, wait for green CI, squash merge PR #61, preserve the branch, keep issue #60 open, and monitor automatic 0.0.22 publication.
- replan_reason = none
<!-- /hard-eng-state -->

## Outcome
- Default circular-dependency analysis omits a confidently classified pure typed GoRouter route-registry ↔ screen navigation cycle.
- Once production code declares a typed GoRouter route, default `check` + `audit` analysis rejects proven raw GoRouter route definitions and destination-navigation calls.
- Genuine non-route dependency cycles remain visible findings.

## Non-goals
- General file-level suppression syntax is unchanged.
- Non-GoRouter cycles + registry-to-registry imports + exports + non-route registry API cycles are not waived.
- Repositories with no typed GoRouter declaration are not forced to migrate in this release.
- `Navigator` page/modal flows + GoRouter `pop` + incoming/deep-link URLs + typed annotation `path:` values are not classified as untyped GoRouter usage.
- Automatic route migration + generated-file creation + `build_runner` execution are excluded.
- Typed routes are not forced into one source file.
- Issue #60 is not closed automatically by this delivery.

## Material decisions
- Pure typed GoRouter cycle policy = accepted by default + no emitted finding.
- Typed-consistency activation = at least one non-generated, non-test typed route declaration in the scanned repository.
- Mixed-routing policy = error on non-test raw `GoRoute` declarations + semantically resolved raw GoRouter destination APIs, including `BuildContext`/`GoRouter` `go`, `push`, `replace`, replacement, named, and location-generation variants.
- Typed navigation = generated route-object `go` + `push` + `replace` + replacement + relative helpers.
- Typed redirect/location = generated route-object `.location`; raw redirect destinations are untyped routing.
- Residual cycle policy = omit only proven typed-navigation back-edges + retain every unrelated residual dependency cycle as an error.
- Receiver proof = imported GoRouter API + resolved `BuildContext`/`GoRouter` receiver; same-named user methods, shadowed types, test code, and `Navigator` APIs remain clean.
- Vendor contract = preserve official `GoRouteData.build` → screen + generated route navigation + generated `.location` redirects.
- Opt-out/config surface = standard reasoned line suppression only; typed consistency is otherwise default after activation.
- Release = next patch version `0.0.22` through the existing GitHub + npm release workflow.
- Approved repository cleanup = remove the terminal issue #58 PLAN; recoverable from merge commit `20c5dcf`.
- Primary evidence = [go_router type-safe routes 17.3.0](https://pub.dev/documentation/go_router/latest/topics/Type-safe%20routes-topic.html) + [go_router_builder 4.4.0](https://pub.dev/packages/go_router_builder).

## Acceptance examples
- Given a route registry whose route builds a screen + that screen navigates with the generated typed route helper, when `cycles` runs, then no circular-dependency finding is emitted.
- Given a multi-screen typed-route navigation loop with only recognized registry ↔ screen helper back-edges, when analysis runs, then no accepted-cycle finding is emitted.
- Given typed routes + a raw `GoRoute`, `context.go(...)`, `router.push(...)`, named navigation, location-generation call, or raw redirect destination in production code, when `check` or `audit` runs, then each proven mixed-routing use is an actionable error.
- Given generated route-object navigation or a generated `.location` redirect, when `check` or `audit` runs, then no untyped-routing finding is emitted.
- Given only raw GoRouter routes/navigation, when `check` or `audit` runs, then the typed-consistency policy emits no finding.
- Given a same-named custom method, `Navigator` flow, `pop`, incoming deep-link string, test fixture, generated file, or shadowed GoRouter symbol, when analysis runs, then no untyped-routing finding is emitted.
- Given a typed-route cycle containing an unrelated import, registry-to-registry edge, export, or non-route registry API use, when analysis runs, then the unrelated residual cycle remains an error.
- Given a normal Dart import cycle without the proven typed-route shape, when analysis runs, then the circular-dependency error is unchanged.

## Affected canonical areas
- Typed GoRouter cycle classification → graph finding emission.
- Route extraction/analysis → typed-adoption + raw-definition/navigation consistency report → stable finding output.
- Focused positive + negative CLI regression fixtures + existing issue #26/#37 route-cycle false-positive corpus.
- README route/cycle policy + synchronized Rust/npm patch-version metadata.
- Terminal feature-record cleanup selected for issue #58.

## Risk and rollback
- risk_level = standard
- critical_overlay = none
- rollback = restore the warning emission for pure typed-route cycles + remove the typed-only consistency finding; restore the issue #58 PLAN from merge commit `20c5dcf`.

## First vertical slice
- S-1 = typed-route adoption + one proven raw GoRouter navigation API → actionable default error while equivalent generated route-object navigation stays clean.
- proof = RED/GREEN CLI contracts for receiver/API positive cases + shadowing/`Navigator`/tests/raw-only negative controls.
