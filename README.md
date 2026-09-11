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
cgtree                           # interactive explorer (falls back to `list` when piped)
cgtree explore                   # explicitly open the interactive explorer
cgtree list                      # print the tree, pipe-friendly
cgtree list --props swap,cpu     # show specific cgroup properties (comma-separated)
cgtree list --props "*"          # show all cgroup properties
cgtree list --depth 2            # limit tree depth
cgtree list --format json        # output as JSON (text or json)
cgtree --root PATH ...           # inspect a different hierarchy root (default /sys/fs/cgroup)
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

**Field Filtering:**

Use `--props` to display cgroup fields alongside the tree. The `--props` option accepts comma-separated field name patterns that use **substring matching**:

```sh
# Show fields containing "memory"
cgtree list --props memory

# Show fields containing "memory" OR "cpu" (substring match)
cgtree list --props memory,cpu

# Show specific fields like memory.swap.max and cpu.weight
cgtree list --props swap,cpu.weight

# Show all fields
cgtree list --props "*"
```

Example output with `--props memory.max,cpu.weight`:

```
/sys/fs/cgroup/
    memory.max = max
    cpu.weight = 100
├── init.scope
│       memory.max = max
│       cpu.weight = 100
└── system.slice/
        memory.max = 1073741824
        cpu.weight = 200
```

Fields are matched by substring, so `--props swap` will match both `memory.swap.max` and `memory.swap.current`.

### Interactive explorer

`cgtree` (or `cgtree explore`) opens an interactive TUI showing the cgroup tree
with expand/collapse navigation and live field filtering.

**Navigation:**

| Key | Action |
| --- | --- |
| `↑`/`↓` or `j`/`k` | move selection |
| `→`/`l`, `←`/`h` | expand / collapse (collapse jumps to parent on a leaf) |
| `Enter`, `Space` | toggle expansion |
| `E` | expand all |
| `C` | collapse all |
| `g` / `G` | jump to top / bottom |
| `?` | show help screen |
| `r` | rescan the hierarchy |
| `q`, `Esc` | quit |

**Properties Display:**

Press `p` to toggle between props display modes:
- **Hide** - No properties shown (default)
- **Show all** - Display all cgroup properties for each node
- **Filtered** - Display only properties matching your saved filter (if you've set one via `f`)

The current mode is shown in the footer (e.g., `p props:hide`, `p props:show-all`, `p props:filtered`).

**Property Filtering:**

Press `f` to activate the property filter. This opens an input prompt at the bottom of the screen where you can type comma-separated property name patterns:

- **Enter** - Apply the filter and display matching properties under each node
- **Esc** - Cancel filter input
- **Substring matching** - Patterns match property names by substring (e.g., `memory` matches `memory.max`, `memory.swap.max`, etc.)
- **Multiple patterns** - Use commas to combine patterns (e.g., `memory,cpu` shows all memory and cpu properties)
- **Show all** - Use `*` to display all properties

**Examples:**

```
f → memory ↵                    # Show all properties containing "memory"
f → memory,cpu ↵                # Show all memory and cpu properties
f → swap,cpu.weight ↵           # Show swap-related properties and cpu.weight
f → * ↵                         # Show all properties
```

When filters are active, the footer displays the current filter (e.g., `f filter [memory,cpu]`). Press `f` again to change the filter.

## Notes

- cgroups are Linux-only; on other platforms (or for testing) point `--root`
  at any directory that mimics a v2 hierarchy — a directory tree where each
  cgroup contains a `cgroup.controllers` file.
- Unreadable cgroup directories (e.g. permission-restricted ones when running
  unprivileged) are shown as leaves rather than aborting the scan.

## Development

### Running Tests

```sh
# Run all tests (unit + integration)
cargo test

# Run only unit tests (inline tests in src/)
cargo test --bins

# Run only integration tests (exercises the actual CLI)
cargo test --test integration_test

# Run with output for debugging
cargo test -- --nocapture

# Run a specific test
cargo test test_name
```

See [`tests/README.md`](tests/README.md) for detailed information about the integration test suite.

### Code Quality

```sh
cargo clippy --all-targets
cargo fmt --check
```
