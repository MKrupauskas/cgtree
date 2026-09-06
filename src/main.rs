mod cgroup;
mod data;
mod list;
mod tui;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Inspect the cgroup v2 hierarchy.
///
/// With no command (or `view`), opens an interactive tree viewer when
/// stdout is a terminal; otherwise prints the tree like `list`.
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

        /// Show the number of processes in each cgroup
        #[arg(short, long)]
        procs: bool,
    },
    /// Open the interactive viewer
    View,
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
        Some(Command::List { depth, procs }) => list::print(&data, depth, procs),
        Some(Command::View) => tui::run(&cli.root, data)?,
        None => {
            if std::io::stdout().is_terminal() {
                tui::run(&cli.root, data)?;
            } else {
                list::print(&data, None, false);
            }
        }
    }
    Ok(())
}
