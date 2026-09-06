use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::data::CgroupData;

/// Errors unless `root` looks like a cgroup v2 unified hierarchy.
pub fn ensure_v2(root: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!("{} does not exist or is not a directory", root.display());
    }
    if root.join("cgroup.controllers").is_file() {
        return Ok(());
    }
    // On a v1 host, /sys/fs/cgroup holds one hierarchy per controller,
    // each with a `tasks` file instead of cgroup.controllers.
    let looks_v1 = ["cpu", "memory", "cpuset", "pids", "blkio", "freezer"]
        .iter()
        .any(|c| root.join(c).join("tasks").is_file());
    if looks_v1 {
        bail!(
            "{} is a cgroup v1 hierarchy; cgtree only supports cgroup v2 (unified)",
            root.display()
        );
    }
    bail!(
        "{} does not look like a cgroup v2 hierarchy (no cgroup.controllers file)",
        root.display()
    );
}

/// Reads the complete cgroup hierarchy with all data.
pub fn scan(root: &Path) -> Result<CgroupData> {
    crate::data::read_all(root)
        .with_context(|| format!("failed to scan {}", root.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Builds a fake cgroup v2 tree: each dir gets cgroup.controllers and
    /// cgroup.procs with the given number of fake pids.
    fn mkgroup(dir: &Path, procs: usize) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        let pids: String = (0..procs).map(|i| format!("{}\n", 100 + i)).collect();
        fs::write(dir.join("cgroup.procs"), pids).unwrap();
    }

    #[test]
    fn scans_v2_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        mkgroup(root, 0);
        mkgroup(&root.join("system.slice/ssh.service"), 1);
        mkgroup(&root.join("system.slice/cron.service"), 2);
        mkgroup(&root.join("user.slice"), 0);
        mkgroup(&root.join("init.scope"), 1);

        ensure_v2(root).unwrap();
        let data = scan(root).unwrap();
        assert_eq!(data.count(), 6);

        let names: Vec<&str> = data.root.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["init.scope", "system.slice", "user.slice"]);

        let system = &data.root.children[1];
        let names: Vec<&str> = system.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["cron.service", "ssh.service"]);
    }

    #[test]
    fn refuses_v1_hierarchy() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for controller in ["cpu", "memory"] {
            let dir = root.join(controller);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("tasks"), "").unwrap();
        }
        let err = ensure_v2(root).unwrap_err().to_string();
        assert!(err.contains("cgroup v1"), "unexpected error: {err}");
    }

    #[test]
    fn refuses_non_cgroup_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let err = ensure_v2(tmp.path()).unwrap_err().to_string();
        assert!(
            err.contains("cgroup.controllers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn refuses_missing_root() {
        let err = ensure_v2(Path::new("/nonexistent/cgtree-test"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not exist"), "unexpected error: {err}");
    }
}
