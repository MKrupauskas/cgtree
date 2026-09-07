mod cgroup;
mod data;
mod filter;
mod list;
mod tui;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
struct Cli {
    /// Root of the cgroup v2 hierarchy
    #[arg(long, global = true, default_value = "/sys/fs/cgroup")]
    root: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
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

        /// Output format (text or json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Open the interactive explorer.
    ///
    /// Press 'f' to filter and display cgroup fields interactively.
    /// Supports comma-separated patterns with substring matching (e.g. "memory,cpu").
    Explore,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("cgtree: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    cgroup::ensure_v2(&cli.root)?;
    let data = cgroup::scan(&cli.root)?;
    match cli.command {
        Some(Command::List {
            depth,
            props,
            format,
        }) => {
            if format == "json" {
                list::print_json(&data, depth, &props)?;
            } else {
                list::print(&data, depth, &props);
            }
        }
        Some(Command::Explore) => tui::run(&cli.root, data)?,
        None => {
            if std::io::stdout().is_terminal() {
                tui::run(&cli.root, data)?;
            } else {
                list::print(&data, None, &[]);
            }
        }
    }
    Ok(())
}
