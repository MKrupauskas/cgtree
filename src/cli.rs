//! The command-line interface definition.
//!
//! This lives in the library rather than `src/main.rs` so that `build.rs` can
//! construct the same `clap::Command` to generate the man page and the shell
//! completion scripts that the Debian package ships. Keeping one definition
//! means the documentation cannot drift from the parser.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Inspect the cgroup v2 hierarchy.
///
/// With no command (or `explore`), opens an interactive tree viewer when
/// stdout is a terminal; otherwise prints the tree like `list`.
///
/// For detailed usage of each command, see:
///   cgtree explore --help
///   cgtree list --help
#[derive(Parser)]
#[command(name = "cgtree", version, about)]
pub struct Cli {
    /// Root of the cgroup v2 hierarchy
    #[arg(long, global = true, default_value = "/sys/fs/cgroup")]
    pub root: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Print the cgroup tree to stdout (pipe-friendly)
    List {
        /// Limit the tree to N levels below the root
        #[arg(short, long)]
        depth: Option<usize>,

        /// Show specified cgroup properties (comma-separated, e.g. "memory.swap.max,cpu.weight").
        /// Patterns use substring matching (e.g. "memory" matches "memory.max", "memory.swap.max").
        /// Use "*" to show all fields.
        #[arg(short, long, value_delimiter = ',')]
        props: Vec<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Open the interactive explorer.
    ///
    /// Press 'f' to filter and display cgroup fields interactively.
    /// Supports comma-separated patterns with substring matching (e.g. "memory,cpu").
    Explore,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub enum Format {
    Text,
    Json,
}
