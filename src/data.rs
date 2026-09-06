use std::path::{Path, PathBuf};

use anyhow::Result;

/// A complete snapshot of cgroup data, including tree structure and all fields.
#[derive(Debug, Clone)]
pub struct CgroupData {
    pub root: CgroupNode,
}

/// A single cgroup in the tree with its metadata and children.
#[derive(Debug, Clone)]
pub struct CgroupNode {
    pub name: String,
    pub path: PathBuf,
    /// Number of pids in cgroup.procs; None if the file was unreadable.
    pub procs: Option<usize>,
    /// All interface files in this cgroup directory (sorted by name).
    pub fields: Vec<FieldEntry>,
    pub children: Vec<CgroupNode>,
}

/// A single cgroup interface file with its name and contents.
#[derive(Debug, Clone)]
pub struct FieldEntry {
    pub name: String,
    pub value: String,
}

impl CgroupData {
    /// Total number of cgroups in this tree.
    pub fn count(&self) -> usize {
        self.root.count()
    }
}

impl CgroupNode {
    /// Total number of cgroups in this subtree, including self.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(CgroupNode::count).sum::<usize>()
    }
}

/// Reads the complete cgroup hierarchy and all interface files upfront.
pub fn read_all(root: &Path) -> Result<CgroupData> {
    let root_node = read_subtree(root, root.display().to_string())?;
    Ok(CgroupData { root: root_node })
}

/// Reads just the tree structure without interface files (faster, composable).
/// Currently unused but kept for future composability.
#[allow(dead_code)]
pub fn read_structure_only(root: &Path) -> Result<CgroupData> {
    let root_node = read_structure_subtree(root, root.display().to_string())?;
    Ok(CgroupData { root: root_node })
}

/// Recursively reads a cgroup subtree with all data.
fn read_subtree(path: &Path, name: String) -> Result<CgroupNode> {
    let mut children = Vec::new();

    // Unreadable directories (e.g. permission denied) are treated as leaves.
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                let child_name = entry.file_name().to_string_lossy().into_owned();
                children.push(read_subtree(&entry.path(), child_name)?);
            }
        }
    }
    children.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(CgroupNode {
        name,
        procs: read_proc_count(path),
        fields: read_interface_files(path),
        path: path.to_path_buf(),
        children,
    })
}

/// Recursively reads a cgroup subtree WITHOUT interface files (structure only).
fn read_structure_subtree(path: &Path, name: String) -> Result<CgroupNode> {
    let mut children = Vec::new();

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                let child_name = entry.file_name().to_string_lossy().into_owned();
                children.push(read_structure_subtree(&entry.path(), child_name)?);
            }
        }
    }
    children.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(CgroupNode {
        name,
        procs: read_proc_count(path),
        fields: Vec::new(), // No fields in structure-only mode
        path: path.to_path_buf(),
        children,
    })
}

/// Reads the number of processes from cgroup.procs.
fn read_proc_count(path: &Path) -> Option<usize> {
    let procs = std::fs::read_to_string(path.join("cgroup.procs")).ok()?;
    Some(procs.lines().filter(|l| !l.trim().is_empty()).count())
}

/// Reads all interface files in a cgroup directory, sorted by name.
fn read_interface_files(path: &Path) -> Vec<FieldEntry> {
    let mut fields = Vec::new();
    let Ok(entries) = std::fs::read_dir(path) else {
        return fields;
    };

    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();

        // Skip write-only files that can trigger side effects
        if name == "memory.reclaim" || name == "cgroup.kill" {
            continue;
        }

        let value = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s.trim_end().to_string(),
            Err(err) => format!("<unreadable: {err}>"),
        };
        fields.push(FieldEntry { name, value });
    }

    fields.sort_by(|a, b| a.name.cmp(&b.name));
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn mkgroup(dir: &Path, procs: usize) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        let pids: String = (0..procs).map(|i| format!("{}\n", 100 + i)).collect();
        fs::write(dir.join("cgroup.procs"), pids).unwrap();
    }

    #[test]
    fn reads_complete_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        mkgroup(root, 0);
        mkgroup(&root.join("system.slice"), 0);
        mkgroup(&root.join("user.slice"), 3);
        fs::write(root.join("memory.current"), "8192\n").unwrap();

        let data = read_all(root).unwrap();
        assert_eq!(data.count(), 3);
        assert_eq!(data.root.children.len(), 2);

        // Check fields were loaded
        let fields: Vec<&str> = data.root.fields.iter().map(|f| f.name.as_str()).collect();
        assert!(fields.contains(&"cgroup.controllers"));
        assert!(fields.contains(&"memory.current"));
    }

    #[test]
    fn structure_only_skips_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        mkgroup(root, 2);
        fs::write(root.join("memory.current"), "4096\n").unwrap();

        let data = read_structure_only(root).unwrap();
        assert_eq!(data.root.procs, Some(2));
        assert_eq!(data.root.fields.len(), 0);
    }

    #[test]
    fn fields_are_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        mkgroup(root, 1);
        fs::write(root.join("z_last"), "z\n").unwrap();
        fs::write(root.join("a_first"), "a\n").unwrap();

        let data = read_all(root).unwrap();
        let names: Vec<&str> = data.root.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names[0], "a_first");
        assert!(names.last() == Some(&"z_last"));
    }
}
