//! The command-line interface definition.
//!
//! This lives in the library rather than `src/main.rs` so that `build.rs` can
//! construct the same `clap::Command` to generate the man page and the shell
//! completion scripts that the Debian package ships. Keeping one definition
//! means the documentation cannot drift from the parser.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Inspect the cgroup v2 hierarchy, or any other sysfs-style directory tree.
///
/// With no command (or `explore`), opens an interactive tree viewer when
/// stdout is a terminal; otherwise prints the tree like `list`.
///
/// `--kind` selects what is inspected: the cgroup v2 hierarchy (default), the
/// DAMON sysfs interface, or any directory tree whose files hold values
/// (sysfs, configfs, debugfs, ...).
///
/// For detailed usage of each command, see:
///   cgtree explore --help
///   cgtree list --help
#[derive(Parser)]
#[command(name = "cgtree", version, about)]
pub struct Cli {
    /// Kind of hierarchy to inspect
    #[arg(short, long, global = true, value_enum, default_value_t = Kind::Cgroup)]
    pub kind: Kind,

    /// Root of the hierarchy [default: /sys/fs/cgroup for `cgroup`,
    /// /sys/kernel/mm/damon/admin for `damon`; required for `generic`]
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Print the tree to stdout (pipe-friendly)
    List {
        /// Limit the tree to N levels below the root
        #[arg(short, long)]
        depth: Option<usize>,

        /// Show specified properties (comma-separated, e.g. "memory.swap.max,cpu.weight").
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
    /// Press 'f' to filter and display properties interactively.
    /// Supports comma-separated patterns with substring matching (e.g. "memory,cpu").
    Explore,
}

/// The kinds of hierarchy cgtree knows how to inspect.
///
/// Every kind is read the same way: directories are nodes and the regular
/// files in them are the node's properties. The kind only decides the default
/// root and how that root is validated before scanning.
#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
pub enum Kind {
    /// cgroup v2 unified hierarchy; refuses cgroup v1
    Cgroup,
    /// DAMON (Data Access MONitor) sysfs interface
    Damon,
    /// Any directory tree, e.g. a sysfs, configfs or debugfs subtree
    Generic,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub enum Format {
    Text,
    Json,
}
