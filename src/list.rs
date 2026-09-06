use crate::data::{CgroupData, CgroupNode};
use anyhow::Result;
use serde::Serialize;

/// Prints the tree in `tree`-style plain text.
pub fn print(data: &CgroupData, depth: Option<usize>, props: &[String]) {
    let mut out = String::new();
    render(data, depth, props, &mut out);
    print!("{out}");
}

/// Prints the tree in JSON format.
pub fn print_json(data: &CgroupData, depth: Option<usize>, props: &[String]) -> Result<()> {
    let json_data = to_json(data, depth, props);
    println!("{}", serde_json::to_string_pretty(&json_data)?);
    Ok(())
}

/// Converts the cgroup data to a JSON-serializable structure.
fn to_json(data: &CgroupData, depth: Option<usize>, props: &[String]) -> JsonOutput {
    JsonOutput {
        root: node_to_json(&data.root, depth, props, 1),
    }
}

/// JSON output structure for the cgroup hierarchy.
#[derive(Serialize)]
struct JsonOutput {
    root: JsonNode,
}

/// JSON representation of a cgroup node.
#[derive(Serialize)]
struct JsonNode {
    name: String,
    path: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<JsonField>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<JsonNode>,
}

/// JSON representation of a cgroup field.
#[derive(Serialize)]
struct JsonField {
    name: String,
    value: String,
}

fn node_to_json(
    node: &CgroupNode,
    depth: Option<usize>,
    props: &[String],
    level: usize,
) -> JsonNode {
    let fields = filter_fields(node, props);
    let children = if depth.is_some_and(|d| level > d) {
        Vec::new()
    } else {
        node.children
            .iter()
            .map(|child| node_to_json(child, depth, props, level + 1))
            .collect()
    };

    JsonNode {
        name: node.name.clone(),
        path: node.path.display().to_string(),
        fields,
        children,
    }
}

fn filter_fields(node: &CgroupNode, props: &[String]) -> Vec<JsonField> {
    if props.is_empty() {
        return Vec::new();
    }

    let show_all = props.iter().any(|p| p == "*");

    if show_all {
        node.fields
            .iter()
            .map(|f| JsonField {
                name: f.name.clone(),
                value: f.value.clone(),
            })
            .collect()
    } else {
        node.fields
            .iter()
            .filter(|field| props.iter().any(|pattern| field.name.contains(pattern.as_str())))
            .map(|f| JsonField {
                name: f.name.clone(),
                value: f.value.clone(),
            })
            .collect()
    }
}

/// Renders the tree into a string (separated from print for testing).
pub fn render(data: &CgroupData, depth: Option<usize>, props: &[String], out: &mut String) {
    out.push_str(&label(&data.root));
    out.push('\n');
    render_props(&data.root, "", props, out);
    render_children(&data.root.children, "", depth, props, 1, out);
}

fn label(node: &CgroupNode) -> String {
    let has_children = !node.children.is_empty();
    if has_children {
        format!("{}/", node.name)
    } else {
        node.name.clone()
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
        out.push_str(&label(node));
        out.push('\n');

        let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
        render_props(node, &child_prefix, props, out);
        render_children(&node.children, &child_prefix, depth, props, level + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn node(name: &str, children: Vec<CgroupNode>) -> CgroupNode {
        node_with_fields(name, Vec::new(), children)
    }

    fn node_with_fields(
        name: &str,
        fields: Vec<(&str, &str)>,
        children: Vec<CgroupNode>,
    ) -> CgroupNode {
        use crate::data::FieldEntry;
        CgroupNode {
            name: name.to_string(),
            path: PathBuf::from(name),
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
                vec![
                    node("init.scope", vec![]),
                    node(
                        "system.slice",
                        vec![node("ssh.service", vec![])],
                    ),
                ],
            ),
        }
    }

    #[test]
    fn renders_tree() {
        let mut out = String::new();
        render(&sample(), None, &[], &mut out);
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
        render(&sample(), Some(1), &[], &mut out);
        assert_eq!(
            out,
            "/sys/fs/cgroup/\n\
             ├── init.scope\n\
             └── system.slice/\n"
        );
    }

    #[test]
    fn shows_props() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                vec![("memory.max", "max"), ("cpu.weight", "100")],
                vec![node_with_fields(
                    "system.slice",
                    vec![("memory.max", "1073741824"), ("cpu.weight", "200")],
                    vec![],
                )],
            ),
        };

        let mut out = String::new();
        render(
            &data,
            None,
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
                vec![
                    ("memory.max", "max"),
                    ("cpu.weight", "100"),
                    ("pids.max", "max"),
                ],
                vec![],
            ),
        };

        let mut out = String::new();
        render(&data, None, &[String::from("*")], &mut out);

        assert!(out.contains("memory.max = max"), "got: {out}");
        assert!(out.contains("cpu.weight = 100"), "got: {out}");
        assert!(out.contains("pids.max = max"), "got: {out}");
    }

    #[test]
    fn displays_multiline_values_with_indentation() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                vec![("memory.stat", "anon 1024\nfile 2048\nshmem 512")],
                vec![],
            ),
        };

        let mut out = String::new();
        render(&data, None, &[String::from("memory.stat")], &mut out);

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
        render(&data, None, &[String::from("swap")], &mut out);

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

    #[test]
    fn outputs_json_hierarchy() {
        let data = sample();
        let json_output = to_json(&data, None, &[]);
        let json_str = serde_json::to_string(&json_output).unwrap();

        // Check that the JSON contains expected structure
        assert!(json_str.contains("\"root\""));
        assert!(json_str.contains("\"name\":\"/sys/fs/cgroup\""));
        assert!(json_str.contains("\"name\":\"init.scope\""));
        assert!(json_str.contains("\"name\":\"system.slice\""));
        assert!(json_str.contains("\"name\":\"ssh.service\""));
    }

    #[test]
    fn outputs_json_with_fields() {
        let data = CgroupData {
            root: node_with_fields(
                "/sys/fs/cgroup",
                vec![("memory.max", "max"), ("cpu.weight", "100")],
                vec![node_with_fields(
                    "system.slice",
                    vec![("memory.max", "1073741824")],
                    vec![],
                )],
            ),
        };

        let json_output = to_json(&data, None, &[String::from("memory.max")]);
        let json_str = serde_json::to_string(&json_output).unwrap();

        // Should include the requested field
        assert!(json_str.contains("\"fields\""));
        assert!(json_str.contains("\"name\":\"memory.max\""));
        assert!(json_str.contains("\"value\":\"max\""));
        assert!(json_str.contains("\"value\":\"1073741824\""));
        // Should NOT include cpu.weight
        assert!(!json_str.contains("cpu.weight"));
    }

    #[test]
    fn json_respects_depth() {
        let data = sample();
        let json_output = to_json(&data, Some(1), &[]);
        let json_str = serde_json::to_string(&json_output).unwrap();

        // Should include first level children
        assert!(json_str.contains("\"name\":\"init.scope\""));
        assert!(json_str.contains("\"name\":\"system.slice\""));
        // Should NOT include second level children
        assert!(!json_str.contains("\"name\":\"ssh.service\""));
    }
}
