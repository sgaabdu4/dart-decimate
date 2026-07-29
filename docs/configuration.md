# Configuration

Dart Decimate reads config from:

1. `.dart-decimaterc`
2. `.dart-decimaterc.json`
3. `.dart-decimaterc.jsonc`
4. `dart-decimate.toml`
5. `.dart-decimate.toml`

Example:

```toml
[cli]
format = "json"
entry = ["lib/main.dart"]
production = true

[health]
max_cyclomatic = 20
max_cognitive = 15
maxUnitSize = 60
coverage_gaps = true
fileScores = true
hotspots = true
targets = true
flutterStyle = true

[dupes]
mode = "semantic"
min_tokens = 80
threshold = 5
ignore_mapper_pairs = true

[flags]
allow = ["SKIP_PERMISSION_PROMPT"]

[boundaries]
presets = ["layered"]
rules = ["lib/domain:lib/ui"]

[security]
surface = true
categories = ["hardcoded-secret", "firebase-api-key", "insecure-transport", "tls-bypass", "weak-randomness"]

[rules]
unused-files = "error"
unused-exports = "warn"
security-candidate = "warn"
"dart-decimate/security-firebase-api-key" = "error"
```

Allowed feature flags remain visible in the inventory but do not create
`feature-flag` findings. Inline suppressions can document intent after `--`,
`because`, or `reason:`, for example:

```dart
// dart-decimate-ignore-next-line feature-flag -- required by E2E startup
```
