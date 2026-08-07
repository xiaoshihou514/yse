# Releasing Yse

## Publishing order

The crates have a strict dependency order. Publish them in this order, bumping
versions with semver-aware changes:

1. `crates/yse-model` — no dependencies beyond std.
2. `crates/yse-ui` — depends on `yse-model` and CXX-Qt.
3. `crates/yse` — facade re-exporting both.
4. `crates/gansi` — depends on nothing in-repo at runtime, but its
   generated templates should move to the published `yse` facade once it is
   available on crates.io.

Before publishing, run the full gate locally:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## CI

No CI workflows are committed (Actions quota is reserved). Validate locally
with the gate above, and add platform CI only when your quota allows.
`gansi build` still produces a Linux release bundle on demand.

## What stays application-specific

Yse cannot automate these; they require your credentials and accounts:

- Code signing (Windows Authenticode, macOS Developer ID).
- macOS notarization and stapling.
- Store submissions (Microsoft Store, App Store, Sparkle).
- Third-party native dependencies beyond Qt.

See the generated `RELEASE.md` inside any `gansi create` project for the
application-side checklist.
