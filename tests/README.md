# Integration tests

Two suites, both of which run on any platform — they build synthetic cgroup v2
hierarchies in a tempdir rather than reading the host's `/sys/fs/cgroup`, so no
Linux host or privileges are required.

| Suite | What it drives | Tests |
| --- | --- | --- |
| `list_test.rs` | The compiled binary, via `std::process::Command` | 28 |
| `explore_test.rs` | The explorer in-process, against a `TestBackend` | 66 |

Unit tests live inline in `src/` (`cargo test --lib`).

```sh
cargo test                      # everything
cargo test --test list_test     # the CLI
cargo test --test explore_test  # the explorer
cargo test -- --nocapture       # with output, for debugging
```

## `list_test.rs`

Runs the real binary and asserts on stdout, stderr, and exit status. `Fixture`
creates a hierarchy in a tempdir; `run(&[args])` invokes the binary (located via
`CARGO_BIN_EXE_cgtree`, so it always matches the current build) and returns
`(stdout, stderr, success)`.

```rust
let f = Fixture::new();
f.add(".", &[]); // the root cgroup, which `ensure_v2` looks for
f.add("system.slice/ssh.service", &[("memory.max", "max")]);

let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list"]);
assert!(ok);
assert!(out.contains("ssh.service"));
```

Covers tree rendering, `--depth`, `--props` (substring matching, `*`, multiple
patterns, multiline values), `--format json`, the error paths (missing root, a
v1 hierarchy, a non-cgroup directory, bad arguments), `--help`/`--version`, the
piped-stdout fallback, and a closed downstream pipe.

## `explore_test.rs`

End-to-end from the user's point of view: each test builds a hierarchy, scans it
with the same code the binary uses, sends real `KeyEvent`s, and asserts on what
a terminal would display. Nothing is stubbed — only the terminal itself is
swapped for ratatui's in-memory `TestBackend`, which is why these tests need no
PTY.

The harness lives in [`harness/mod.rs`](harness/mod.rs). It is a subdirectory
module rather than `harness.rs` because files directly under `tests/` are each
compiled as their own test binary.

```rust
let f = Fixture::new();
f.add("system.slice/ssh.service", &[]);

let mut tui = f.explore(80, 24);
tui.press('j').press('l'); // select system.slice, expand it
assert_eq!(tui.selected().trim(), "└── system.slice/");
assert!(tui.text().contains("ssh.service"));
```

- `Fixture::explore(w, h)` — open the explorer at a terminal size.
- `press(c)` / `key(code)` / `type_str(s)` — send keys; each is followed by a
  redraw, mirroring the real event loop.
- `tree()` / `footer()` / `text()` / `visible()` — read the rendered screen. The
  volatile tempdir path is normalised to `<root>` so assertions stay stable.
- `selected()` / `selected_index()` — the highlighted row, found via its
  selection background: the same cue a user sees.

Covers the initial render, navigation (`jk`, arrows, `g`/`G`, `Home`/`End`, and
the bounds), expand/collapse (`Enter`, `Space`, `hl`, `E`/`C`), tree-guide
drawing at depth, the props cycle (`p`), property filtering (`f` — substring,
comma-separated, `*`, empty, no-match, editing, cancelling), multiline and empty
values, the help screen (`?`), quitting (`q`, `Esc`, `Ctrl-C`, and the cases
where those keys must *not* quit), refresh (`r`), and edge cases such as a
root-only hierarchy, unknown keys, many siblings, and an undersized terminal.

## Conventions

Give each test a fresh `Fixture` — they share no state and can run in parallel.
Name tests for the behaviour under test (`list_props_substring_match`,
`filter_input_cancelled_with_esc`), assert on both what should and should not
appear, and prefer an exact `assert_eq!` on the rendered output over a
`contains` check when the full line is what matters.
