//! Integration tests that run the actual cgtree CLI binary.
//!
//! These tests exercise the CLI by running the compiled binary against
//! temporary cgroup v2 hierarchies, ensuring end-to-end correctness.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

// ============================================================================
// Test Helpers
// ============================================================================

/// Creates a fake cgroup v2 hierarchy in a temporary directory.
struct Fixture {
    #[allow(dead_code)]
    tempdir: TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let root = tempdir.path().to_path_buf();
        Self { tempdir, root }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    /// Creates a cgroup directory with cgroup.controllers and optional fields.
    fn add(&self, path: &str, fields: &[(&str, &str)]) {
        let dir = self.root.join(path);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        fs::write(dir.join("cgroup.procs"), "1000\n").unwrap();
        for (name, value) in fields {
            fs::write(dir.join(name), format!("{}\n", value)).unwrap();
        }
    }
}

/// Runs cgtree with args, returns (stdout, stderr, success).
fn run(args: &[&str]) -> (String, String, bool) {
    let bin = env!("CARGO_BIN_EXE_cgtree");
    let output = Command::new(bin).args(args).output().unwrap();
    (
        String::from_utf8_lossy(&output.stdout).into(),
        String::from_utf8_lossy(&output.stderr).into(),
        output.status.success(),
    )
}

// ============================================================================
// List Command Tests
// ============================================================================

#[test]
fn list_basic_tree() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("init.scope", &[]);
    f.add("system.slice", &[]);
    f.add("system.slice/ssh.service", &[]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list"]);

    assert!(ok, "command should succeed");

    // Assert exact output structure
    let expected = format!(
        "{}/\n├── init.scope\n└── system.slice/\n    └── ssh.service\n",
        f.root().display()
    );
    assert_eq!(out, expected);
}

#[test]
fn list_with_depth() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("level1", &[]);
    f.add("level1/level2", &[]);
    f.add("level1/level2/level3", &[]);

    

    // Depth 1: only show level1
    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--depth", "1"]);
    assert!(ok);
    let expected = format!("{}/\n└── level1/\n", f.root().display());
    assert_eq!(out, expected);

    // Depth 2: show up to level2
    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--depth", "2"]);
    assert!(ok);
    let expected = format!("{}/\n└── level1/\n    └── level2/\n", f.root().display());
    assert_eq!(out, expected);
}

#[test]
fn list_with_props() {
    let f = Fixture::new();
    f.add(".", &[("memory.max", "max"), ("cpu.weight", "100")]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--props", "memory.max"]);
    assert!(ok);

    // Root without children doesn't get trailing slash
    let expected = format!("{}\n    memory.max = max\n", f.root().display());
    assert_eq!(out, expected);
}

#[test]
fn list_props_substring_match() {
    let f = Fixture::new();
    f.add(".", &[("memory.swap.max", "123"), ("memory.max", "456")]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--props", "swap"]);
    assert!(ok);

    
    let expected = format!("{}\n    memory.swap.max = 123\n", f.root().display());
    assert_eq!(out, expected);
}

#[test]
fn list_props_star() {
    let f = Fixture::new();
    f.add(".", &[("memory.max", "max"), ("cpu.weight", "100")]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--props", "*"]);
    assert!(ok);

    
    let expected = format!(
        "{}\n    cgroup.controllers = cpu memory pids\n    cgroup.procs = 1000\n    cpu.weight = 100\n    memory.max = max\n",
        f.root().display()
    );
    assert_eq!(out, expected);
}

#[test]
fn list_multiline_values() {
    let f = Fixture::new();
    f.add(".", &[("memory.stat", "anon 1024\nfile 2048")]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--props", "memory.stat"]);
    assert!(ok);


    let expected = format!(
        "{}\n    memory.stat =\n        anon 1024\n        file 2048\n",
        f.root().display()
    );
    assert_eq!(out, expected);
}

#[test]
fn list_multiple_prop_filters() {
    let f = Fixture::new();
    f.add(".", &[
        ("memory.max", "max"),
        ("memory.swap.max", "max"),
        ("cpu.weight", "100"),
        ("cpu.max", "100000 100000"),
        ("pids.max", "max"),
    ]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--props", "memory.max,cpu.weight"]);
    assert!(ok);

    // Should show both memory.max and cpu.weight, but not others
    let expected = format!(
        "{}\n    cpu.weight = 100\n    memory.max = max\n",
        f.root().display()
    );
    assert_eq!(out, expected);
}

#[test]
fn list_no_props_by_default() {
    let f = Fixture::new();
    f.add(".", &[("memory.max", "max"), ("cpu.weight", "100")]);
    f.add("child", &[]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list"]);
    assert!(ok);

    // Should show tree but no properties without --props flag
    let expected = format!("{}/\n└── child\n", f.root().display());
    assert_eq!(out, expected);
}

// ============================================================================
// JSON Output Tests
// ============================================================================

#[test]
fn json_output() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("child", &[]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--format", "json"]);
    assert!(ok);

    let json: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");

    // Verify exact structure
    assert_eq!(json["root"]["name"], f.root().display().to_string());
    assert_eq!(json["root"]["path"], f.root().display().to_string());

    let children = json["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "child");
}

#[test]
fn json_with_props() {
    let f = Fixture::new();
    f.add(".", &[("memory.max", "max")]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--format", "json", "--props", "memory.max"]);
    assert!(ok);

    let json: serde_json::Value = serde_json::from_str(&out).unwrap();

    let fields = json["root"]["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0]["name"], "memory.max");
    assert_eq!(fields[0]["value"], "max");
}

#[test]
fn json_respects_depth() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("a", &[]);
    f.add("a/b", &[]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--format", "json", "--depth", "1"]);
    assert!(ok);

    let json: serde_json::Value = serde_json::from_str(&out).unwrap();

    let children = json["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "a");

    // Should not have grandchildren due to depth limit
    let grandchildren = children[0].get("children");
    assert!(grandchildren.is_none() || grandchildren.unwrap().as_array().unwrap().is_empty());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn error_nonexistent_root() {
    let (out, err, ok) = run(&["--root", "/nonexistent/path", "list"]);
    assert!(!ok);
    assert_eq!(out, "");
    assert_eq!(err, "cgtree: /nonexistent/path does not exist or is not a directory\n");
}

#[test]
fn error_not_cgroup() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().to_str().unwrap();
    let (out, err, ok) = run(&["--root", path, "list"]);
    assert!(!ok);
    assert_eq!(out, "");
    assert_eq!(
        err,
        format!("cgtree: {} does not look like a cgroup v2 hierarchy (no cgroup.controllers file)\n", path)
    );
}

#[test]
fn error_cgroup_v1() {
    let tempdir = tempfile::tempdir().unwrap();
    let root = tempdir.path();

    // Create fake v1 hierarchy
    for controller in ["cpu", "memory"] {
        let dir = root.join(controller);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("tasks"), "").unwrap();
    }

    let (out, err, ok) = run(&["--root", root.to_str().unwrap(), "list"]);
    assert!(!ok);
    assert_eq!(out, "");
    assert_eq!(
        err,
        format!("cgtree: {} is a cgroup v1 hierarchy; cgtree only supports cgroup v2 (unified)\n", root.display())
    );
}

#[test]
fn error_invalid_depth() {
    let f = Fixture::new();
    f.add(".", &[]);

    let (out, err, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--depth", "not-a-number"]);
    assert!(!ok);
    assert_eq!(out, "");
    // Error message contains "invalid" and mentions the value
    assert!(err.contains("invalid") || err.contains("parse"));
    assert!(err.contains("not-a-number"));
}

// ============================================================================
// Edge Cases and Robustness
// ============================================================================

#[test]
fn empty_root() {
    let f = Fixture::new();
    f.add(".", &[]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list"]);
    assert!(ok);


    assert_eq!(out, format!("{}\n", f.root().display()));
}

#[test]
fn depth_zero() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("child1", &[]);
    f.add("child2", &[]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--depth", "0"]);
    assert!(ok);

    // Depth 0 should show only the root (with trailing slash since it has children), no children listed
    assert_eq!(out, format!("{}/\n", f.root().display()));
}

#[test]
fn deep_hierarchy() {
    let f = Fixture::new();
    f.add(".", &[]);

    // Create 10 levels deep
    let mut path = String::new();
    for i in 0..10 {
        if i > 0 { path.push('/'); }
        path.push_str(&format!("l{}", i));
        f.add(&path, &[]);
    }

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list"]);
    assert!(ok);
    assert!(out.contains("l0") && out.contains("l9"));
}

#[test]
fn many_children() {
    let f = Fixture::new();
    f.add(".", &[]);

    for i in 0..50 {
        f.add(&format!("child{:02}", i), &[]);
    }

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list"]);
    assert!(ok);
    assert!(out.contains("child00") && out.contains("child49"));
}

#[test]
fn special_characters_in_values() {
    let f = Fixture::new();
    f.add(".", &[("test", "value with spaces\tand\ttabs")]);

    let (out, _, _) = run(&["--root", f.root().to_str().unwrap(), "list", "--props", "*"]);
    assert!(out.contains("value with spaces"));
}

#[test]
fn very_long_values() {
    let f = Fixture::new();
    let long_value = "x".repeat(10000);
    f.add(".", &[("long.field", &long_value)]);

    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list", "--props", "long.field"]);
    assert!(ok);

    // Should handle very long values without crashing
    let expected = format!("{}\n    long.field = {}\n", f.root().display(), long_value);
    assert_eq!(out, expected);
}

#[test]
fn concurrent_runs() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("child", &[]);

    let root = f.root().to_str().unwrap().to_string();
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let r = root.clone();
            std::thread::spawn(move || run(&["--root", &r, "list"]))
        })
        .collect();

    for handle in handles {
        let (_, _, ok) = handle.join().unwrap();
        assert!(ok);
    }
}

// ============================================================================
// CLI Behavior Tests
// ============================================================================

#[test]
fn help_flag() {
    let (out, err, ok) = run(&["--help"]);
    assert!(ok);
    assert_eq!(err, "");
    // Help text contains version which is dynamic, so we check key parts
    assert!(out.contains("Inspect the cgroup v2 hierarchy"));
    assert!(out.contains("list"));
    assert!(out.contains("explore"));
    assert!(out.contains("--root"));
}

#[test]
fn version_flag() {
    let (out, err, ok) = run(&["--version"]);
    assert!(ok);
    assert_eq!(err, "");
    // Version is dynamic, but format is stable
    assert!(out.starts_with("cgtree "));
    assert!(out.contains("0.1.0"));
}

#[test]
fn default_behavior() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("child", &[]);

    // No subcommand should work like list in non-TTY
    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap()]);
    assert!(ok);

    
    let expected = format!("{}/\n└── child\n", f.root().display());
    assert_eq!(out, expected);
}
