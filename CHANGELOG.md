# Changelog

All notable changes to Yse will be documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## Unreleased

## 0.1.1 - 2026-08-17

### Fixed

- Made Qt SDK installation preserve executable modes and symbolic links, place
  ICU libraries correctly, retry downloads, and stop when required modules fail.
- Made Linux bundles resolve real shared libraries, include Qt and ICU runtime
  dependencies, skip unusable plugins, remain relocatable, and avoid treating
  unrelated libraries beside a system Qt installation as Qt-owned runtime.
- Made Windows builds discover qmake and windeployqt from gansi-managed Qt roots
  and deploy complete runnable Qt bundles.
- Fixed project creation inside workspaces and Windows MSVC toolchain detection.
- Made 7z symbolic-link detection independent of host path separators.

### Changed

- Generated projects use the published `yse` facade and include their own
  workspace boundary.
- Added reproducible native Windows check, application smoke, and bundle smoke
  workflows, including isolated execution without the Qt SDK on `PATH`.

## 0.1.0 - 2026-08-11

### Added

- Pure-Rust transactional reactive runtime and Qt 6 Widgets integration.
- `gansi` project creation, diagnostics, build, and bundle commands.
- Linux, Windows, and headless smoke examples.

### Stability

Yse is pre-release. Breaking API changes are allowed before the first public
release and will be recorded here.
