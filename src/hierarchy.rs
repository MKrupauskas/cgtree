//! Picks, validates and scans the hierarchy selected with `--kind`.
//!
//! Scanning is the same for every kind (see `data::read_all`); what differs is
//! where the hierarchy lives by default and how we confirm that the root is
//! the thing the user asked for before reading it.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::cgroup;
use crate::cli::Kind;
use crate::data::CgroupData;

/// The root inspected when `--root` is not given, if the kind has one.
pub fn default_root(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Cgroup => Some("/sys/fs/cgroup"),
        Kind::Damon => Some("/sys/kernel/mm/damon/admin"),
        Kind::Generic => None,
    }
}

/// Resolves the root to inspect: `--root` if given, else the kind's default.
pub fn resolve_root(kind: Kind, root: Option<PathBuf>) -> Result<PathBuf> {
    match (root, default_root(kind)) {
        (Some(root), _) => Ok(root),
        (None, Some(default)) => Ok(PathBuf::from(default)),
        (None, None) => bail!("--kind generic needs --root PATH to know what to inspect"),
    }
}

/// Errors unless `root` looks like a hierarchy of the given kind.
pub fn validate(kind: Kind, root: &Path) -> Result<()> {
    match kind {
        Kind::Cgroup => cgroup::ensure_v2(root),
        Kind::Damon => ensure_damon(root),
        Kind::Generic => ensure_dir(root),
    }
}

/// Reads the complete hierarchy under `root` with all data.
pub fn scan(root: &Path) -> Result<CgroupData> {
    crate::data::read_all(root).with_context(|| format!("failed to scan {}", root.display()))
}

fn ensure_dir(root: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!("{} does not exist or is not a directory", root.display());
    }
    Ok(())
}

/// The DAMON sysfs admin directory always holds a `kdamonds` directory, even
/// before any monitoring thread has been configured.
fn ensure_damon(root: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!(
            "{} does not exist; the DAMON sysfs interface needs a kernel built \
             with CONFIG_DAMON_SYSFS (and sysfs mounted at /sys)",
            root.display()
        );
    }
    if !root.join("kdamonds").is_dir() {
        bail!(
            "{} does not look like the DAMON sysfs admin directory (no kdamonds directory)",
            root.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_roots() {
        assert_eq!(
            resolve_root(Kind::Cgroup, None).unwrap(),
            Path::new("/sys/fs/cgroup")
        );
        assert_eq!(
            resolve_root(Kind::Damon, None).unwrap(),
            Path::new("/sys/kernel/mm/damon/admin")
        );
        let err = resolve_root(Kind::Generic, None).unwrap_err().to_string();
        assert!(err.contains("--root"), "unexpected error: {err}");
    }

    #[test]
    fn explicit_root_wins() {
        let root = resolve_root(Kind::Damon, Some(PathBuf::from("/tmp/x"))).unwrap();
        assert_eq!(root, Path::new("/tmp/x"));
    }

    #[test]
    fn damon_needs_kdamonds() {
        let tmp = tempfile::tempdir().unwrap();
        let err = validate(Kind::Damon, tmp.path()).unwrap_err().to_string();
        assert!(err.contains("kdamonds"), "unexpected error: {err}");

        fs::create_dir(tmp.path().join("kdamonds")).unwrap();
        validate(Kind::Damon, tmp.path()).unwrap();
    }

    #[test]
    fn damon_missing_root_mentions_kernel_config() {
        let err = validate(Kind::Damon, Path::new("/nonexistent/cgtree-damon"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("CONFIG_DAMON_SYSFS"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn generic_accepts_any_directory() {
        let tmp = tempfile::tempdir().unwrap();
        validate(Kind::Generic, tmp.path()).unwrap();
        let err = validate(Kind::Generic, &tmp.path().join("missing"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not exist"), "unexpected error: {err}");
    }
}
