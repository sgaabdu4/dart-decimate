# CI

Add Dart Decimate to CI so every PR gets the same repo health check:

```yaml
- name: Dart Decimate
  run: npx --yes dart-decimate json .
```

That is the easiest CI command. It checks everything Dart Decimate knows how to
check in one pass.

For PR-only regression checks, use:

```bash
npx --yes dart-decimate audit . --base origin/main --format json --summary --gate new-only
```

You can also put the full check in a git hook:

```bash
mkdir -p .git/hooks
cat > .git/hooks/pre-commit <<'SH'
#!/usr/bin/env sh
npx --yes dart-decimate json .
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
`test`, `lint`, and `format` commands. The checked-in pre-push hook uses
`DART_DECIMATE_BASE_REF` or `origin/main`, fetches a missing remote base, and
runs the same lint/test stack before allowing a push.

Run the complete verification stack locally:

```bash
git diff --check
npm ci --ignore-scripts
npm run lint
npm run version:bump:check -- origin/main
npm run release:check
cargo fmt --check
cargo clippy --all-targets -- -D warnings
npx fallow audit --base origin/main --quiet
cargo test --all-targets
npm test
npm run pack:check
npm run test:postinstall:prebuilt
npm run test:npx:prebuilt
npm run test:npx:local
npm run test:npx:mcp:local
```

Generate CI templates:

```bash
dart-decimate ci-template github --format yaml
dart-decimate ci-template gitlab --format yaml
```

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
