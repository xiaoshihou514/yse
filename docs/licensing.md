# Licensing and Qt distribution

Yse source code is offered under `MIT OR Apache-2.0`. CXX-Qt, CXX-Qt-Lib,
and CXX are also offered under `MIT OR Apache-2.0`. Those licenses do not
replace or weaken the separate license terms of Qt or any Qt third-party
components that an application uses or redistributes.

This document is an engineering checklist, not legal advice. Application
distributors must review the exact Qt edition, version, modules, plugins, and
third-party components in their artifacts and obtain qualified legal advice
when needed.

## Choose a Qt license path

Before distributing an application, choose one Qt licensing path and record it
in the application's release documentation:

- **Commercial Qt:** use Qt under the commercial agreement that applies to the
  development team and distribution. Do not assume an open-source Qt install
  can automatically be converted into a commercial build.
- **Open-source Qt:** comply with the license of every shipped Qt component.
  Most essential Qt 6 modules are available under LGPLv3/GPLv3, while some
  modules are GPL-only for open-source users and bundled third-party code has
  its own terms.

Qt's official guidance says wrappers do not shield applications from Qt's
license and recommends dynamic linking for proprietary applications using the
LGPL path. Open-source distribution also requires, among other obligations:

1. Prominent notice that the application uses Qt and a copy of the applicable
   license terms.
2. A distributor-controlled way to obtain the complete corresponding source
   for the exact LGPL Qt libraries supplied, including modifications, or an
   appropriate written offer.
3. Preservation of the user's right to replace or modify the Qt libraries and
   run the relinked application; application terms must not forbid the reverse
   engineering needed for that purpose.
4. Review of every shipped Qt module, plugin, and third-party component because
   not all components use the same license.

Official references:

- <https://www.qt.io/development/open-source-lgpl-obligations>
- <https://www.qt.io/faq/qt-open-source-licensing>
- <https://doc.qt.io/qt-6/licensing.html>

## Yse packaging policy

- Yse links Qt dynamically by default and does not support static Qt packaging
  as the default open-source distribution path.
- `gansi build` writes `THIRD_PARTY_NOTICES.txt`, Yse's MIT and Apache-2.0
  license texts, discoverable Cargo package license/notice files,
  `QT_RUNTIME.tsv`, `SYSTEM_RUNTIME.tsv`, `GLIBC_REQUIREMENTS.tsv`, and the Qt
  license directory discoverable from the
  installed SDK into Linux bundles. The system inventory exposes non-Qt shared
  dependencies for separate license and compatibility review; it does not copy
  them.
  These artifacts support review but do not substitute for Qt source provision,
  relinking requirements, or license texts absent from that SDK.
- Linux deployment includes the selected Qt shared libraries and plugins even
  during offscreen smoke testing, so the same payload shape is exercised before
  release.
- A release must record the exact Qt version and deployed shared libraries and
  plugins. Review the resulting bundle rather than relying only on declared
  Cargo dependencies.
- `gansi setup` extracts a downloaded Qt archive only after validating its
  archive-specific `.sha256` sidecar, or its `.sha1` sidecar when SHA-256 is
  unavailable. Missing, malformed, and mismatched checksums reject the archive.

## CXX-Qt and Rust dependencies

CXX-Qt and CXX are linked into Yse applications under their `MIT OR
Apache-2.0` terms. Preserve their copyright and license notices in distributed
source or binary notice materials as required by the selected license. Rust
dependencies can change between releases, so produce a license inventory from
the release `Cargo.lock` and review packages with missing, non-SPDX, copyleft,
or otherwise unexpected terms.
