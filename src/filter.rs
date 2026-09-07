use crate::data::{CgroupNode, FieldEntry};

/// Parses a comma-separated string into field name patterns.
/// Example: "memory,cpu.weight" -> ["memory", "cpu.weight"]
pub fn parse_field_patterns(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Filters fields from a node based on the given patterns.
/// - If patterns is empty, returns empty vec
/// - If patterns contains "*", returns all fields
/// - Otherwise returns fields that contain any of the patterns as a substring
pub fn filter_node_fields<'a>(
    node: &'a CgroupNode,
    patterns: &[String],
) -> Vec<&'a FieldEntry> {
    if patterns.is_empty() {
        return Vec::new();
    }

    let show_all = patterns.iter().any(|p| p == "*");

    if show_all {
        node.fields.iter().collect()
    } else {
        node.fields
            .iter()
            .filter(|field| patterns.iter().any(|pattern| field.name.contains(pattern.as_str())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn node_with_fields(name: &str, fields: Vec<(&str, &str)>) -> CgroupNode {
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
            children: vec![],
        }
    }

    #[test]
    fn parse_empty_string() {
        let patterns = parse_field_patterns("");
        assert_eq!(patterns, Vec::<String>::new());
    }

    #[test]
    fn parse_single_pattern() {
        let patterns = parse_field_patterns("memory");
        assert_eq!(patterns, vec!["memory"]);
    }

    #[test]
    fn parse_multiple_patterns() {
        let patterns = parse_field_patterns("memory,cpu,pids");
        assert_eq!(patterns, vec!["memory", "cpu", "pids"]);
    }

    #[test]
    fn parse_with_whitespace() {
        let patterns = parse_field_patterns("memory, cpu.weight , pids");
        assert_eq!(patterns, vec!["memory", "cpu.weight", "pids"]);
    }

    #[test]
    fn filter_empty_patterns() {
        let node = node_with_fields("test", vec![("memory.max", "max")]);
        let fields = filter_node_fields(&node, &[]);
        assert_eq!(fields.len(), 0);
    }

    #[test]
    fn filter_with_star() {
        let node = node_with_fields(
            "test",
            vec![
                ("memory.max", "max"),
                ("cpu.weight", "100"),
                ("pids.max", "max"),
            ],
        );
        let fields = filter_node_fields(&node, &[String::from("*")]);
        assert_eq!(fields.len(), 3);
    }

    #[test]
    fn filter_by_substring() {
        let node = node_with_fields(
            "test",
            vec![
                ("memory.swap.max", "max"),
                ("memory.swap.current", "0"),
                ("memory.max", "1073741824"),
                ("cpu.weight", "100"),
            ],
        );
        let fields = filter_node_fields(&node, &[String::from("swap")]);
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.name == "memory.swap.max"));
        assert!(fields.iter().any(|f| f.name == "memory.swap.current"));
    }

    #[test]
    fn filter_by_multiple_patterns() {
        let node = node_with_fields(
            "test",
            vec![
                ("memory.swap.max", "max"),
                ("memory.max", "1073741824"),
                ("cpu.weight", "100"),
                ("cpu.max", "100000 100000"),
            ],
        );
        let fields = filter_node_fields(
            &node,
            &[String::from("swap"), String::from("cpu.weight")],
        );
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.name == "memory.swap.max"));
        assert!(fields.iter().any(|f| f.name == "cpu.weight"));
    }
}
