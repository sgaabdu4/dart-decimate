# CI

Add Dart Decimate to CI so every PR gets the same repo health check:

```yaml
- name: Dart Decimate
  run: npx --yes dart-decimate@0.0.54 --strict
```

That is the easiest CI command. It checks everything Dart Decimate knows how to
check in one pass, uses the default zero duplication threshold, and fails for
warnings as well as errors.

For PR-only regression checks, use:

```bash
npx --yes dart-decimate@0.0.54 audit . --base origin/main --format json --summary --gate new-only
```

`--gate new-only` limits the finding gate to issues introduced by the change.
The zero duplication threshold remains an absolute repository rule, including
inside the audit scope. Use a reviewed duplication baseline when adopting the
gate in a repository that already contains known clones.

You can also put the full check in a git hook:

```bash
mkdir -p .git/hooks
cat > .git/hooks/pre-commit <<'SH'
#!/usr/bin/env sh
npx --yes dart-decimate@0.0.54 --strict
SH
chmod +x .git/hooks/pre-commit
```

This repository already runs:

- Rust format, clippy, and tests
- npm package checks
- version sync between `Cargo.toml` and `package.json`
- a PR version-bump gate requiring both package files to increase to an
  unpublished version
- release guards that reject reused npm versions or tags on different commits
- migration checks that block previous package, command, schema, and MCP names
- Fallow audit against the base branch
- Dependabot and weekly dependency/security audits

Local gate settings live in `.no-mistakes.yaml`. They allow three auto-fix
attempts for rebase, review, test, document, lint, and CI work, with deterministic
`test`, `lint`, and `format` commands. Hard Eng owns the full gate: the
checked-in pre-push hook and the `Rust and npm checks` CI job both run
`python3 .hooks/hard-eng.py check`, which runs every check listed in
`hard-eng.gates.json`.

Run the complete verification stack locally:

```bash
python3 .hooks/hard-eng.py check
```

Generate CI templates:

```bash
dart-decimate ci-template github --format yaml
dart-decimate ci-template gitlab --format yaml
```

Generated templates intentionally use `--strict`: any visible warning in the
changed-code audit scope fails CI, alongside errors and the zero duplication
threshold.

Preview review-thread reconciliation without changing GitHub or GitLab:

```bash
dart-decimate ci reconcile-review \
  --provider github \
  --repo owner/repo \
  --pr 123 \
  --envelope review-github.json \
  --dry-run \
  --format json
```
