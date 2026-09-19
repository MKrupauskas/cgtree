# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Debian packaging: `cargo deb` builds a `.deb` for amd64 and arm64, shipping
  the binary, man pages, and bash/zsh/fish completions.
- Man pages and shell completions generated at build time from the clap
  definition.

## [0.1.0]

### Added

- `list` command: prints the cgroup v2 hierarchy as a tree, with `--depth`,
  `--props` field filtering, and `--format text|json`.
- `explore` command: interactive terminal explorer with live field filtering.
- `--root` for inspecting a hierarchy other than `/sys/fs/cgroup`.

[Unreleased]: https://github.com/MKrupauskas/cgtree/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/MKrupauskas/cgtree/releases/tag/v0.1.0
