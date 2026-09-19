use cgtree::cli::{Cli, Command, Format};
use cgtree::{cgroup, list, tui};

use std::io::IsTerminal;
use std::process::ExitCode;

use clap::Parser;

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
        }) => match format {
            Format::Json => list::print_json(&data, depth, &props)?,
            Format::Text => list::print(&data, depth, &props)?,
        },
        Some(Command::Explore) => tui::run(&cli.root, data)?,
        None => {
            if std::io::stdout().is_terminal() {
                tui::run(&cli.root, data)?;
            } else {
                list::print(&data, None, &[])?;
            }
        }
    }
    Ok(())
}
