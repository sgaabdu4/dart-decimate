# Issue Types

Run this for the live list:

```bash
dart-decimate schema --format json | jq .issue_types
```

Current issue types:

```text
dead-file
unused-export
unused-type
private-type-leak
unused-enum-member
unused-class-member
duplicate-export
route-collision
private-widget-class
widget-top-level-function-boundary
unused-widget-param
unrendered-widget
missing-context-mounted-after-await
missing-entry-point
circular-dependency
re-export-cycle
boundary-violation
boundary-coverage
boundary-call-violation
policy-violation
unresolved-dependency
part-of-violation
unused-dependency
unused-dev-dependency
test-only-dependency
unused-dependency-override
misconfigured-dependency-override
unlisted-dependency
private-src-import
code-duplication
high-cyclomatic-complexity
high-cognitive-complexity
high-complexity
coverage-gap
high-crap-score
health-hotspot
refactoring-target
feature-flag
security-candidate
stale-suppression
missing-suppression-reason
```
