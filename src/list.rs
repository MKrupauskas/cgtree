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
            out.push_str(prefix);
            out.push_str("    ");
            out.push_str(&field.name);
            out.push_str(" = ");
            // Collapse multi-line values into a single line
            let value = collapse_value(&field.value);
            out.push_str(&value);
            out.push('\n');
        }
    } else {
        for prop in props {
            if let Some(field) = node.fields.iter().find(|f| &f.name == prop) {
                out.push_str(prefix);
                out.push_str("    ");
                out.push_str(prop);
                out.push_str(" = ");
                // Collapse multi-line values into a single line
                let value = collapse_value(&field.value);
                out.push_str(&value);
                out.push('\n');
            }
        }
    }
}

fn collapse_value(value: &str) -> String {
    let lines: Vec<&str> = value.lines().collect();
    if lines.len() <= 1 {
        value.to_string()
    } else {
        lines.join("\\n")
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
    fn collapses_multiline_values() {
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

        assert!(out.contains("memory.stat = anon 1024\\nfile 2048\\nshmem 512"), "got: {out}");
        // Ensure no actual newlines within the value
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2); // Only root name and the property line
    }
}
