## What changed

- 

## Validation

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --all-targets`
- [ ] `bun run version:check`
- [ ] `bun run version:bump:check origin/main`
- [ ] `bun run release:check`
- [ ] `bun run migration:check`
- [ ] `bun run pack:check`

## Release

- [ ] `Cargo.toml` and `package.json` versions match
- [ ] This PR bumps both `Cargo.toml` and `package.json` to an unpublished `dart-decimate` version, unless native Hard Eng verification proves a canonical scaffold-only update
