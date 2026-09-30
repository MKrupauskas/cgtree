# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `--kind cgroup|damon|generic` (`-k`) to inspect hierarchies other than
  cgroup v2. `damon` reads the DAMON sysfs interface at
  `/sys/kernel/mm/damon/admin`; `generic` reads any directory tree given with
  `--root`, such as a sysfs, configfs or debugfs subtree. `cgroup` stays the
  default, and `--root` now defaults to the chosen kind's usual location.

### Changed

- Symlinks are listed as properties showing their target (`-> ../devices/...`)
  instead of being ignored. They are still never followed.
- Files with no read permission bits (write-only kernel triggers) are skipped.
- Files are opened non-blocking, values are capped at 64 KiB, and non-UTF-8
  contents show as `<binary: N bytes>`, so scanning arbitrary kernel
  interfaces cannot hang or flood the output.

## [0.1.0] - 2026-09-19

First tagged release.

### Added

- `list` command: prints the cgroup v2 hierarchy as a tree, with `--depth`,
  `--props` field filtering, and `--format text|json`.
- `explore` command: interactive terminal explorer with live field filtering.
- `--root` for inspecting a hierarchy other than `/sys/fs/cgroup`.
- Debian packages for amd64 and arm64, published on each tagged release.
  Statically linked against musl, so they have no runtime dependencies and
  work across Debian and Ubuntu releases.
- Man pages (`cgtree`, `cgtree-list`, `cgtree-explore`) and bash, zsh and
  fish completions, generated from the CLI definition and shipped in the
  package.

[Unreleased]: https://github.com/MKrupauskas/cgtree/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/MKrupauskas/cgtree/releases/tag/v0.1.0
