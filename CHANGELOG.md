# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-09-19

First release of `cgtree`, a CLI for inspecting the cgroup v2 hierarchy on a
host. It prints the layout under `/sys/fs/cgroup` as a tree, optionally
annotated with cgroup properties such as memory and cpu limits, so that
debugging resource isolation or comparing slice layout across hosts does not
mean walking the tree by hand.

Running `cgtree` with no arguments opens the interactive explorer when stdout
is a terminal, and falls back to `list` output when piped, so the same command
works interactively and in a shell pipeline.

### Added

#### Commands

- `list`: prints the hierarchy as a `tree`-style listing on stdout,
  pipe-friendly. `--depth N` limits the tree to N levels below the root, and
  `--format text|json` selects between the tree rendering and a JSON document
  containing every node, its path, and all of its interface files — suitable
  for feeding into `jq` or another tool.
- `explore`: an interactive terminal explorer for walking large hierarchies,
  with expand/collapse navigation (arrow keys or `j`/`k`/`h`/`l`), expand-all
  and collapse-all, jump to top/bottom, rescan with `r`, and an in-app help
  screen on `?`.

#### Property display and filtering

- `--props` on `list` displays cgroup interface files alongside each node. It
  takes comma-separated patterns matched by substring, so `--props swap,cpu`
  shows `memory.swap.max`, `memory.swap.current` and every `cpu.*` field;
  `--props "*"` shows all of them.
- The explorer applies the same matching live: `p` cycles the property display
  between hidden, all properties, and the saved filter, and `f` opens a prompt
  for editing that filter without leaving the tree.

#### Hierarchy selection and robustness

- `--root PATH` inspects a hierarchy other than `/sys/fs/cgroup` — a mounted
  namespace, a copy captured from another host, or a synthetic tree in tests.
- cgroup v1 hosts are detected and reported as such rather than producing a
  confusing partial tree; `list` refuses to run against them.
- Unreadable cgroup directories, such as permission-restricted ones when
  running unprivileged, are rendered as leaves instead of aborting the scan.

#### Packaging

- Debian packages for amd64 and arm64, published on each tagged release.
  Statically linked against musl, so they have no runtime dependencies and
  work across Debian and Ubuntu releases.
- Man pages (`cgtree`, `cgtree-list`, `cgtree-explore`) and bash, zsh and
  fish completions, generated from the CLI definition and shipped in the
  package.

[Unreleased]: https://github.com/MKrupauskas/cgtree/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/MKrupauskas/cgtree/releases/tag/v0.1.0
