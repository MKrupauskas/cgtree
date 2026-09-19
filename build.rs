//! Generates the man page and shell completion scripts at build time.
//!
//! The Debian package ships both, and `cargo deb` picks them up from
//! `target/dist/`. Generating them from the same `clap::Command` the binary
//! parses with means they cannot drift from the real interface.
//!
//! A build script cannot link against its own crate's library, so the CLI
//! definition is compiled in a second time here as a module. The
//! `cargo:rerun-if-changed` lines below keep the generated files in step with
//! edits to it.

use std::env;
use std::fs;
use std::io::Error;
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::Shell;

#[path = "src/cli.rs"]
mod cli;

use cli::Cli;

fn main() -> Result<(), Error> {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=build.rs");

    // Place artifacts in a fixed location rather than under the per-build
    // OUT_DIR hash, so packaging and CI can reference them by a stable path.
    let dist = match env::var_os("CARGO_MANIFEST_DIR") {
        Some(dir) => PathBuf::from(dir).join("target").join("dist"),
        None => return Ok(()),
    };
    fs::create_dir_all(&dist)?;

    let mut cmd = Cli::command();
    cmd.build();

    let mut page = Vec::new();
    clap_mangen::Man::new(cmd.clone()).render(&mut page)?;
    fs::write(dist.join("cgtree.1"), page)?;

    // Subcommand man pages (cgtree-list.1, cgtree-explore.1) so that
    // `man cgtree-list` works the way the top-level --help points users.
    // clap's built-in `help` subcommand is skipped: a man page restating
    // "print this message" is noise, and Debian's lintian flags man pages
    // that no binary or documented command corresponds to.
    let version = env!("CARGO_PKG_VERSION");
    for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
        let name = format!("cgtree-{}", sub.get_name());
        let mut page = Vec::new();
        clap_mangen::Man::new(sub.clone().name(name.clone()).version(version)).render(&mut page)?;
        fs::write(dist.join(format!("{name}.1")), page)?;
    }

    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
        clap_complete::generate_to(shell, &mut cmd, "cgtree", &dist)?;
    }

    Ok(())
}
