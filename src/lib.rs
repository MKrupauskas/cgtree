//! Internals of the `cgtree` binary, exposed as a library.
//!
//! `src/main.rs` is a thin CLI wrapper over these modules. They live in a
//! library target because Rust integration tests (`tests/`) are compiled as
//! separate crates and can therefore only reach a crate's public API — this
//! is what lets `tests/explore_test.rs` drive the interactive explorer
//! directly, feeding it keystrokes and reading back the rendered terminal
//! buffer.
//!
//! This is not a general-purpose published API; it is public only as far as
//! the binary and the test suite require.

pub mod cgroup;
pub mod list;
pub mod tui;

pub(crate) mod data;
pub(crate) mod filter;
