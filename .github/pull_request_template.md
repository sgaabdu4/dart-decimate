## What changed

- 

## Validation

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --all-targets`
- [ ] `pnpm run version:check`
- [ ] `pnpm run version:bump:check origin/main`
- [ ] `pnpm run release:check`
- [ ] `pnpm run migration:check`
- [ ] `pnpm run pack:check`

## Release

- [ ] `Cargo.toml` and `package.json` versions match
- [ ] This PR bumps both `Cargo.toml` and `package.json` to an unpublished `dart-decimate` version, unless native Hard Eng verification proves a canonical scaffold-only update
