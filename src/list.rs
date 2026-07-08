use crate::cgroup::Node;

/// Prints the tree in `tree`-style plain text.
pub fn print(tree: &Node, depth: Option<usize>, procs: bool) {
    let mut out = String::new();
    render(tree, depth, procs, &mut out);
    print!("{out}");
}

/// Renders the tree into a string (separated from print for testing).
pub fn render(tree: &Node, depth: Option<usize>, procs: bool, out: &mut String) {
    out.push_str(&label(tree, procs));
    out.push('\n');
    render_children(&tree.children, "", depth, procs, 1, out);
}

fn label(node: &Node, procs: bool) -> String {
    match (procs, node.procs) {
        (true, Some(n)) => format!("{} ({n})", node.name),
        (true, None) => format!("{} (?)", node.name),
        (false, _) => node.name.clone(),
    }
}

fn render_children(
    nodes: &[Node],
    prefix: &str,
    depth: Option<usize>,
    procs: bool,
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
        render_children(&node.children, &child_prefix, depth, procs, level + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn node(name: &str, procs: Option<usize>, children: Vec<Node>) -> Node {
        Node {
            name: name.to_string(),
            path: PathBuf::from(name),
            procs,
            children,
        }
    }

    fn sample() -> Node {
        node(
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
        )
    }

    #[test]
    fn renders_tree() {
        let mut out = String::new();
        render(&sample(), None, false, &mut out);
        assert_eq!(
            out,
            "/sys/fs/cgroup\n\
             ├── init.scope\n\
             └── system.slice\n\
             \u{20}   └── ssh.service\n"
        );
    }

    #[test]
    fn respects_depth() {
        let mut out = String::new();
        render(&sample(), Some(1), false, &mut out);
        assert_eq!(
            out,
            "/sys/fs/cgroup\n\
             ├── init.scope\n\
             └── system.slice\n"
        );
    }

    #[test]
    fn shows_proc_counts() {
        let mut out = String::new();
        render(&sample(), Some(1), true, &mut out);
        assert!(out.contains("init.scope (1)"), "got: {out}");
    }
}
