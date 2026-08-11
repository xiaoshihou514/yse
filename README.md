# Yse

Rust-first framework for building reactive Qt 6 Widgets desktop applications
without QML.

Rust 优先的框架，用于构建无需 QML 的反应式 Qt 6 Widgets 桌面应用。

## Documentation / 文档

- [English / 英文](README.en.md)
- [中文](README.zh.md)

## Crates

- [`yse-model`](crates/yse-model) — pure-Rust reactive runtime (no Qt, no
  `unsafe`) / 纯 Rust 反应式运行时（盐水鹅模型）
- [`yse-ui`](crates/yse-ui) — reactive Qt Widgets layer / 反应式 Qt
  Widgets 层
- [`yse`](crates/yse) — facade re-exporting both layers / 同时再导出两层
  的 facade（盐水鹅）
- [`gansi`](crates/gansi) — developer toolchain CLI / 开发者工具链 CLI（干丝）

See [README.en.md](README.en.md) or [README.zh.md](README.zh.md) for the
full guide.

完整指南见 [README.en.md](README.en.md) 或 [README.zh.md](README.zh.md)。
