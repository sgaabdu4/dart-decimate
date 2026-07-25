# Feature Brief: Issue 58 Clean Architecture Patterns

<!-- hard-eng-state:v1 -->
- state_version = 1
- plan_id = issue-58-clean-architecture-patterns-1875a335
- lifecycle_status = green
- approval_status = approved
- approval_fingerprint = sha256:91f97a91002b76a560a22154fd2490c34cfefb1f4d1448f230e79eb429fb7e66
- approval_provenance = ready-to-build
- green_artifact = sha256:50ca3815717e89bf7abc5b3df6ccef856547911679688fc8b53153bffb559401
- active_slice = none
- completed_slices = S-1
- next_action = Update PR 59 with the qualified-return and shared parser fixes, then wait for required CI.
- replan_reason = none
<!-- /hard-eng-state -->

## Outcome
- Default duplicate-code analysis recognises intentional class pairs joined by an explicit `toEntity()` or `toDomain()` mapper and omits only that pair's clone finding.
- Projects can opt out and restore mapper-pair clone findings through duplicate-code configuration.
- Intentional feature flags remain acknowledgeable by name while staying visible in inventory; unacknowledged flags remain findings by default.

## Non-goals
- Arbitrary `toX()` methods do not establish mapper boundaries.
- Test-source feature flags are not acknowledged automatically.
- Path-pair ignore globs + general duplication thresholds + rule severity semantics remain unchanged.
- Mapper handling does not hide clone groups containing an unrelated third declaration.

## Material decisions
- Mapper method set = exact `toEntity` + `toDomain`.
- Mapper target = another project class identified by the mapper's declared return type.
- Mapper suppression default = enabled.
- Mapper suppression opt-out = `[dupes] ignore_mapper_pairs = false` + JSONC alias `ignoreMapperPairs`.
- Feature-flag acknowledgement = existing `[flags] allow = [...]`; allowed names remain inventory-only + unallowed names remain blocking findings.

## Acceptance examples
- Given duplicated fields in `UserModel` + `UserEntity` joined by `UserEntity toEntity()`, when duplicate analysis uses defaults, then their pair-only clone is omitted.
- Given duplicated fields in classes without `Model`/`Entity` suffixes joined by `Order toDomain()`, when duplicate analysis uses defaults, then their pair-only clone is omitted.
- Given the same mapper pair + `ignore_mapper_pairs = false`, when duplicate analysis runs, then the clone is reported.
- Given duplicated classes joined only by `toJson()` or another arbitrary `toX()`, when duplicate analysis runs, then the clone remains reported.
- Given a clone group spanning a valid mapper pair + an unrelated third declaration, when duplicate analysis runs, then the group remains reported.
- Given one allowed + one unallowed compile-time feature flag, when `check` runs, then both remain in inventory + only the unallowed flag creates a finding.

## Affected canonical areas
- Duplicate analyzer options + mapper-boundary filter + config deserialization.
- CLI/config schema contracts + README configuration guidance.
- Focused duplicate/config regression tests + existing feature-flag contract tests.

## Risk and rollback
- risk_level = standard
- critical_overlay = none
- rollback = set `dupes.ignore_mapper_pairs = false` for immediate project-level recovery; code rollback removes the option + restores the current narrow filter.

## First vertical slice
- S-1 = default-on explicit mapper recognition + config opt-out → duplicate report behavior + documented configuration.
- proof = RED/GREEN public CLI fixtures for `toEntity` + `toDomain` + opt-out + arbitrary-method + third-declaration controls; existing feature-flag allow/inventory contract remains green.
