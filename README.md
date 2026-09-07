# cgtree

Small CLI for inspecting the cgroup hierarchy on a host—useful when you want a
structured view of how cgroups are laid out under `/sys/fs/cgroup` without
walking the tree by hand. Typical uses include debugging resource isolation,
comparing slice layout across hosts, and piping output into other tools for
quick exploration.

The tool targets cgroup v2 unified hierarchies; it refuses to run `list` on
cgroup v1.

## Install

```sh
cargo install --path .
```

## Usage

```sh
cgtree                  # interactive explorer (falls back to `list` when piped)
cgtree list             # print the tree, pipe-friendly
cgtree list --props swap,cpu  # show specific cgroup properties
cgtree list --depth 2   # limit tree depth
cgtree --root PATH ...  # inspect a different hierarchy root (default /sys/fs/cgroup)
```

### `list`

Plain `tree`-style output on stdout:

```
/sys/fs/cgroup/
├── init.scope
├── system.slice/
│   ├── cron.service
│   └── ssh.service
└── user.slice/
    └── user-1000.slice/
```

### Interactive explorer

`cgtree` (or `cgtree explore`) opens an interactive TUI showing the cgroup tree
with expand/collapse navigation.

| Key | Action |
| --- | --- |
| `↑`/`↓` or `j`/`k` | move selection |
| `→`/`l`, `←`/`h` | expand / collapse (collapse jumps to parent on a leaf) |
| `Enter`, `Space` | toggle expansion |
| `g` / `G` | jump to top / bottom |
| `r` | rescan the hierarchy |
| `q`, `Esc` | quit |

## Notes

- cgroups are Linux-only; on other platforms (or for testing) point `--root`
  at any directory that mimics a v2 hierarchy — a directory tree where each
  cgroup contains a `cgroup.controllers` file.
- Unreadable cgroup directories (e.g. permission-restricted ones when running
  unprivileged) are shown as leaves rather than aborting the scan.

## Development

```sh
cargo test
cargo clippy --all-targets
```
