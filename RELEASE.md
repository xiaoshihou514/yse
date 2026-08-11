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
CCACHE_DISABLE=1 just release-check
```

This command does not publish or upload anything. It validates the Fedora-local
workspace, MSRV, dependency licenses, standalone crate packages, generated
application lifecycle, Linux release bundle, examples, and Valgrind teardown.
It is not evidence for another operating system or an older Linux distribution.

## CI

No CI workflows are committed (Actions quota is reserved). Validate locally
with the gate above, and add platform CI only when your quota allows.
`gansi build` still produces a Linux release bundle on demand.
That bundle carries its Qt libraries and selected plugins behind a relocatable
launcher and records them in `QT_RUNTIME.tsv`; it does not bundle glibc,
graphics drivers, or every non-Qt system dependency. Validate it on the oldest
Linux distribution you intend to support and review `SYSTEM_RUNTIME.tsv` for
the remaining compatibility and licensing surface. The adjacent architecture-
labelled `.tar.gz` is the transport artifact for the complete bundle directory;
verify its adjacent `SHA256SUMS` entry after transfer.

## What stays application-specific

Yse cannot automate these; they require your credentials and accounts:

- Code signing (Windows Authenticode, macOS Developer ID).
- macOS notarization and stapling.
- Store submissions (Microsoft Store, App Store, Sparkle).
- Third-party native dependencies beyond Qt.

See the generated `RELEASE.md` inside any `gansi create` project for the
application-side checklist.
