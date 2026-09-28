use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

/// A complete snapshot of cgroup data, including tree structure and all fields.
#[derive(Debug, Clone, Serialize)]
pub struct CgroupData {
    pub root: CgroupNode,
}

/// A single cgroup in the tree with its metadata and children.
#[derive(Debug, Clone, Serialize)]
pub struct CgroupNode {
    pub name: String,
    pub path: PathBuf,
    /// All interface files in this cgroup directory (sorted by name).
    pub fields: Vec<FieldEntry>,
    pub children: Vec<CgroupNode>,
}

/// A single cgroup interface file with its name and contents.
#[derive(Debug, Clone, Serialize)]
pub struct FieldEntry {
    pub name: String,
    pub value: String,
}

impl CgroupData {
    /// Total number of cgroups in this tree.
    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.root.count()
    }
}

#[cfg(test)]
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
        fields: read_interface_files(path),
        path: path.to_path_buf(),
        children,
    })
}

/// Largest value read from a single file. sysfs attributes are at most a
/// page, but binary attributes and debugfs files can be arbitrarily large.
const MAX_VALUE_BYTES: usize = 64 * 1024;

/// Reads all interface files in a directory, sorted by name.
///
/// Symlinks are listed with their target rather than followed: in sysfs they
/// point across the tree (and back up it), so following them would duplicate
/// subtrees or loop forever.
fn read_interface_files(path: &Path) -> Vec<FieldEntry> {
    let mut fields = Vec::new();
    let Ok(entries) = std::fs::read_dir(path) else {
        return fields;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();

        if file_type.is_symlink() {
            let value = match std::fs::read_link(entry.path()) {
                Ok(target) => format!("-> {}", target.display()),
                Err(err) => format!("<unreadable link: {err}>"),
            };
            fields.push(FieldEntry { name, value });
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        // Skip write-only files that can trigger side effects
        if name == "memory.reclaim" || name == "cgroup.kill" || is_write_only(&entry) {
            continue;
        }

        let value = match read_value(&entry.path()) {
            Ok(s) => s,
            Err(err) => format!("<unreadable: {err}>"),
        };
        fields.push(FieldEntry { name, value });
    }

    fields.sort_by(|a, b| a.name.cmp(&b.name));
    fields
}

/// Whether nobody may read the file. Kernel interfaces use this mode for
/// trigger files (cgroup.kill, memory.reclaim, many sysfs knobs) that have no
/// value to show.
fn is_write_only(entry: &std::fs::DirEntry) -> bool {
    use std::os::unix::fs::PermissionsExt;
    entry
        .metadata()
        .is_ok_and(|m| m.permissions().mode() & 0o444 == 0)
}

/// Reads one file's value, trimmed of trailing whitespace.
///
/// The file is opened non-blocking so that stream-like files (tracefs
/// `trace_pipe`, some debugfs entries) return at once instead of hanging the
/// scan waiting for data. Values over `MAX_VALUE_BYTES` are cut short, and
/// contents that are not UTF-8 are summarised rather than printed.
fn read_value(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_VALUE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let truncated = bytes.len() > MAX_VALUE_BYTES;
    bytes.truncate(MAX_VALUE_BYTES);

    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        // The cut may have split a multi-byte character; keep what precedes it.
        Err(err) if truncated && err.error_len().is_none() => {
            std::str::from_utf8(&bytes[..err.valid_up_to()]).expect("valid up to here")
        }
        Err(_) if truncated => return Ok(format!("<binary: over {MAX_VALUE_BYTES} bytes>")),
        Err(_) => return Ok(format!("<binary: {} bytes>", bytes.len())),
    };
    if truncated {
        Ok(format!(
            "{}\n<truncated at {MAX_VALUE_BYTES} bytes>",
            text.trim_end()
        ))
    } else {
        Ok(text.trim_end().to_string())
    }
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

    #[test]
    fn symlinks_are_listed_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("devices/dev0")).unwrap();
        fs::create_dir_all(root.join("class")).unwrap();
        // A link back up the tree would loop forever if followed.
        std::os::unix::fs::symlink("../devices/dev0", root.join("class/dev0")).unwrap();
        std::os::unix::fs::symlink("..", root.join("devices/dev0/parent")).unwrap();

        let data = read_all(root).unwrap();
        assert_eq!(data.count(), 4, "links are not scanned as directories");
        let class = &data.root.children[0];
        assert_eq!(class.name, "class");
        assert_eq!(class.fields[0].name, "dev0");
        assert_eq!(class.fields[0].value, "-> ../devices/dev0");
    }

    #[test]
    fn write_only_files_are_skipped() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("trigger"), "").unwrap();
        fs::set_permissions(root.join("trigger"), fs::Permissions::from_mode(0o200)).unwrap();
        fs::write(root.join("state"), "off\n").unwrap();

        let data = read_all(root).unwrap();
        let names: Vec<&str> = data.root.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["state"]);
    }

    #[test]
    fn binary_values_are_summarised() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("blob"), [0xff, 0xfe, 0x00, 0x01]).unwrap();

        let data = read_all(tmp.path()).unwrap();
        assert_eq!(data.root.fields[0].value, "<binary: 4 bytes>");
    }

    #[test]
    fn large_values_are_truncated() {
        let tmp = tempfile::tempdir().unwrap();
        // A multi-byte character straddling the cut must not read as binary.
        let mut big = "a".repeat(MAX_VALUE_BYTES - 1);
        big.push_str(&"é".repeat(10));
        fs::write(tmp.path().join("big"), &big).unwrap();

        let data = read_all(tmp.path()).unwrap();
        let value = &data.root.fields[0].value;
        assert!(
            value.ends_with("<truncated at 65536 bytes>"),
            "{}",
            &value[value.len() - 40..]
        );
        assert!(value.starts_with(&"a".repeat(MAX_VALUE_BYTES - 1)));
    }
}
