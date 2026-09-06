use crate::data::{CgroupData, CgroupNode};

/// Prints the tree in `tree`-style plain text.
pub fn print(data: &CgroupData, depth: Option<usize>, procs: bool, props: &[String]) {
    let mut out = String::new();
    render(data, depth, procs, props, &mut out);
    print!("{out}");
}

/// Renders the tree into a string (separated from print for testing).
pub fn render(data: &CgroupData, depth: Option<usize>, procs: bool, props: &[String], out: &mut String) {
    out.push_str(&label(&data.root, procs));
    out.push('\n');
    render_props(&data.root, "", props, out);
    render_children(&data.root.children, "", depth, procs, props, 1, out);
}

fn label(node: &CgroupNode, procs: bool) -> String {
    let has_children = !node.children.is_empty();
    let name = if has_children {
        format!("{}/", node.name)
    } else {
        node.name.clone()
    };

    match (procs, node.procs) {
        (true, Some(n)) => format!("{name} ({n})"),
        (true, None) => format!("{name} (?)"),
        (false, _) => name,
    }
}

fn render_props(node: &CgroupNode, prefix: &str, props: &[String], out: &mut String) {
    if props.is_empty() {
        return;
    }

    // Check if user wants all props with "*"
    let show_all = props.iter().any(|p| p == "*");

    if show_all {
        for field in &node.fields {
            render_prop(&field.name, &field.value, prefix, out);
        }
    } else {
        // Collect matching fields (supports substring matching)
        let mut matched_fields = Vec::new();
        for field in &node.fields {
            for pattern in props {
                if field.name.contains(pattern.as_str()) {
                    matched_fields.push(field);
                    break; // Avoid duplicates if multiple patterns match
                }
            }
        }

        for field in matched_fields {
            render_prop(&field.name, &field.value, prefix, out);
        }
    }
}

fn render_prop(name: &str, value: &str, prefix: &str, out: &mut String) {
    let lines: Vec<&str> = value.lines().collect();

    if lines.is_empty() {
        // Empty value
        out.push_str(prefix);
        out.push_str("    ");
        out.push_str(name);
        out.push_str(" = \n");
    } else if lines.len() == 1 {
        // Single-line value
        out.push_str(prefix);
        out.push_str("    ");
        out.push_str(name);
        out.push_str(" = ");
        out.push_str(value);
        out.push('\n');
    } else {
        // Multi-line value: indent each line
        out.push_str(prefix);
        out.push_str("    ");
        out.push_str(name);
        out.push_str(" =\n");
        for line in lines {
            out.push_str(prefix);
            out.push_str("        ");
            out.push_str(line);
            out.push('\n');
        }
    }
}

fn render_children(
    nodes: &[CgroupNode],
    prefix: &str,
    depth: Option<usize>,
    procs: bool,
    props: &[String],
    level: usize,
    out: &mut String,
) {
    if depth.is_some_and(|d| level > d) {
        return;
    }
    for (i, node) in nodes.iter().enumerate() {
        let last = i == nodes.len() - 1;
        out.push_str(prefix);
        out.push_str(if last { "└── " } else { "├── " });
        out.push_str(&label(node, procs));
        out.push('\n');

        let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
        render_props(node, &child_prefix, props, out);
        render_children(&node.children, &child_prefix, depth, procs, props, level + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn node(name: &str, procs: Option<usize>, children: Vec<CgroupNode>) -> CgroupNode {
        node_with_fields(name, procs, Vec::new(), children)
    }

    fn node_with_fields(
        name: &str,
        procs: Option<usize>,
        fields: Vec<(&str, &str)>,
        children: Vec<CgroupNode>,
    ) -> CgroupNode {
        use crate::data::FieldEntry;
        CgroupNode {
            name: name.to_string(),
            path: PathBuf::from(name),
            procs,
            fields: fields
                .into_iter()
                .map(|(k, v)| FieldEntry {
                    name: k.to_string(),
                    value: v.to_string(),
                })
                .collect(),
            children,
        }
    }

    fn sample() -> CgroupData {
        CgroupData {
            root: node(
                "/sys/fs/cgroup",
                Some(0),
                vec![
                    node("init.scope", Some(1), vec![]),
                    node(
                        "system.slice",
                        Some(0),
                        vec![node("ssh.service", Some(2), vec![])],
                    ),
                ],
            ),
        }
    }

    #[test]
    fn renders_tree() {
        let mut out = String::new();
        render(&sample(), None, false, &[], &mut out);
        assert_eq!(
            out,
            "/sys/fs/cgroup/\n\
             ├── init.scope\n\
             └── system.slice/\n\
             \u{20}   └── ssh.service\n"
        );
    }

    #[test]
    fn respects_depth() {
        let mut out = String::new();
        render(&sample(), Some(1), false, &[], &mut out);
        assert_eq!(
            out,
            "/sys/fs/cgroup/\n\
             ├── init.scope\n\
             └── system.slice/\n"
        );
    }

    #[test]
    fn shows_proc_counts() {
        let mut out = String::new();
        render(&sample(), Some(1), true, &[], &mut out);
        assert!(out.contains("init.scope (1)"), "got: {out}");
    }

    #[test]
    fn shows_props() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                Some(0),
                vec![("memory.max", "max"), ("cpu.weight", "100")],
                vec![node_with_fields(
                    "system.slice",
                    Some(0),
                    vec![("memory.max", "1073741824"), ("cpu.weight", "200")],
                    vec![],
                )],
            ),
        };

        let mut out = String::new();
        render(
            &data,
            None,
            false,
            &[String::from("memory.max"), String::from("cpu.weight")],
            &mut out,
        );

        assert!(out.contains("memory.max = max"), "got: {out}");
        assert!(out.contains("cpu.weight = 100"), "got: {out}");
        assert!(out.contains("memory.max = 1073741824"), "got: {out}");
        assert!(out.contains("cpu.weight = 200"), "got: {out}");
    }

    #[test]
    fn shows_all_props_with_star() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                Some(0),
                vec![
                    ("memory.max", "max"),
                    ("cpu.weight", "100"),
                    ("pids.max", "max"),
                ],
                vec![],
            ),
        };

        let mut out = String::new();
        render(&data, None, false, &[String::from("*")], &mut out);

        assert!(out.contains("memory.max = max"), "got: {out}");
        assert!(out.contains("cpu.weight = 100"), "got: {out}");
        assert!(out.contains("pids.max = max"), "got: {out}");
    }

    #[test]
    fn displays_multiline_values_with_indentation() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                Some(0),
                vec![("memory.stat", "anon 1024\nfile 2048\nshmem 512")],
                vec![],
            ),
        };

        let mut out = String::new();
        render(&data, None, false, &[String::from("memory.stat")], &mut out);

        assert!(out.contains("memory.stat =\n"), "got: {out}");
        assert!(out.contains("        anon 1024\n"), "got: {out}");
        assert!(out.contains("        file 2048\n"), "got: {out}");
        assert!(out.contains("        shmem 512\n"), "got: {out}");
    }

    #[test]
    fn matches_props_by_substring() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                Some(0),
                vec![
                    ("memory.swap.max", "max"),
                    ("memory.swap.current", "0"),
                    ("memory.max", "1073741824"),
                    ("cpu.weight", "100"),
                ],
                vec![],
            ),
        };

        let mut out = String::new();
        render(&data, None, false, &[String::from("swap")], &mut out);

        // Should match both memory.swap.max and memory.swap.current
        assert!(out.contains("memory.swap.max = max"), "got: {out}");
        assert!(out.contains("memory.swap.current = 0"), "got: {out}");
        // Should NOT match memory.max or cpu.weight
        assert!(!out.contains("memory.max = "), "got: {out}");
        assert!(!out.contains("cpu.weight = "), "got: {out}");
    }

    #[test]
    fn matches_multiple_patterns() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                Some(0),
                vec![
                    ("memory.swap.max", "max"),
                    ("memory.max", "1073741824"),
                    ("cpu.weight", "100"),
                    ("cpu.max", "100000 100000"),
                ],
                vec![],
            ),
        };

        let mut out = String::new();
        render(
            &data,
            None,
            false,
            &[String::from("swap"), String::from("cpu.weight")],
            &mut out,
        );

        // Should match memory.swap.max and cpu.weight
        assert!(out.contains("memory.swap.max = max"), "got: {out}");
        assert!(out.contains("cpu.weight = 100"), "got: {out}");
        // Should NOT match memory.max or cpu.max
        assert!(!out.contains("memory.max = "), "got: {out}");
        assert!(!out.contains("cpu.max = "), "got: {out}");
    }

    #[test]
    fn matches_multiple_broad_patterns() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                Some(0),
                vec![
                    ("memory.swap.max", "max"),
                    ("memory.swap.current", "0"),
                    ("memory.max", "1073741824"),
                    ("cpu.weight", "100"),
                    ("cpu.max", "100000 100000"),
                    ("cpu.stat", "usage_usec 12345\nuser_usec 6789"),
                    ("pids.max", "max"),
                ],
                vec![],
            ),
        };

        let mut out = String::new();
        render(
            &data,
            None,
            false,
            &[String::from("swap"), String::from("cpu")],
            &mut out,
        );

        // Should match all swap and cpu properties
        assert!(out.contains("memory.swap.max = max"), "got: {out}");
        assert!(out.contains("memory.swap.current = 0"), "got: {out}");
        assert!(out.contains("cpu.weight = 100"), "got: {out}");
        assert!(out.contains("cpu.max = 100000 100000"), "got: {out}");
        assert!(out.contains("cpu.stat =\n"), "got: {out}");
        // Should NOT match memory.max or pids.max
        assert!(!out.contains("memory.max = "), "got: {out}");
        assert!(!out.contains("pids.max = "), "got: {out}");
    }
}
