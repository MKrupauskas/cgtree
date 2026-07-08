use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// A cgroup directory and its descendants.
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    /// Number of pids in cgroup.procs; None if the file was unreadable.
    pub procs: Option<usize>,
    pub children: Vec<Node>,
}

impl Node {
    /// Total number of cgroups in this subtree, including self.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Node::count).sum::<usize>()
    }
}

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

/// Walks the hierarchy under `root` into a tree of Nodes.
pub fn scan(root: &Path) -> Result<Node> {
    build(root, root.display().to_string())
        .with_context(|| format!("failed to scan {}", root.display()))
}

fn build(path: &Path, name: String) -> Result<Node> {
    let mut children = Vec::new();
    // Unreadable directories (e.g. permission denied as non-root) are
    // treated as leaves rather than aborting the whole scan.
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                let name = entry.file_name().to_string_lossy().into_owned();
                children.push(build(&entry.path(), name)?);
            }
        }
    }
    children.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Node {
        name,
        procs: proc_count(path),
        path: path.to_path_buf(),
        children,
    })
}

fn proc_count(path: &Path) -> Option<usize> {
    let procs = fs::read_to_string(path.join("cgroup.procs")).ok()?;
    Some(procs.lines().filter(|l| !l.trim().is_empty()).count())
}

/// Reads every interface file directly inside a cgroup directory,
/// sorted by name. Unreadable files report the error as their value.
pub fn read_fields(path: &Path) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let Ok(entries) = fs::read_dir(path) else {
        return fields;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let value = match fs::read_to_string(entry.path()) {
            Ok(s) => s.trim_end().to_string(),
            Err(err) => format!("<unreadable: {err}>"),
        };
        fields.push((name, value));
    }
    fields.sort();
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let tree = scan(root).unwrap();
        assert_eq!(tree.count(), 6);

        let names: Vec<&str> = tree.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["init.scope", "system.slice", "user.slice"]);

        let system = &tree.children[1];
        let names: Vec<&str> = system.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["cron.service", "ssh.service"]);
        assert_eq!(system.children[0].procs, Some(2));
        assert_eq!(system.children[1].procs, Some(1));
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

    #[test]
    fn reads_fields_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        mkgroup(root, 1);
        fs::write(root.join("memory.current"), "4096\n").unwrap();

        let fields = read_fields(root);
        let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["cgroup.controllers", "cgroup.procs", "memory.current"]
        );
        assert_eq!(fields[2].1, "4096");
    }
}
