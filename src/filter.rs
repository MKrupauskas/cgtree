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
/// - Otherwise returns fields whose name matches any of the patterns
///   (see [`matches_pattern`])
pub fn filter_node_fields<'a>(node: &'a CgroupNode, patterns: &[String]) -> Vec<&'a FieldEntry> {
    node.fields
        .iter()
        .filter(|field| {
            patterns
                .iter()
                .any(|pattern| matches_pattern(&field.name, pattern))
        })
        .collect()
}

/// Reports whether a field name matches a single pattern.
///
/// A pattern containing `*` or `?` is a glob matched against the whole name:
/// `*` matches any run of characters and `?` exactly one (so `memory*.max`
/// matches `memory.max` and `memory.swap.max`, and a bare `*` matches every
/// field). Any other pattern matches as a substring.
pub fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern.contains(['*', '?']) {
        glob_match(name, pattern)
    } else {
        name.contains(pattern)
    }
}

/// Matches `name` against a glob made of literal characters, `*` and `?`.
///
/// Iterative backtracking: on a mismatch, retry from the most recent `*`
/// letting it absorb one more character. Linear in practice, and never worse
/// than O(name × pattern).
fn glob_match(name: &str, pattern: &str) -> bool {
    let name: Vec<char> = name.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    let (mut n, mut p) = (0, 0);
    // Position of the last `*` in the pattern, and the name position it was
    // tried against.
    let mut star: Option<(usize, usize)> = None;

    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, n));
                p += 1;
            }
            Some(&c) if c == '?' || c == name[n] => {
                n += 1;
                p += 1;
            }
            _ => match star {
                Some((sp, sn)) => {
                    p = sp + 1;
                    n = sn + 1;
                    star = Some((sp, sn + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
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
        let fields = filter_node_fields(&node, &[String::from("swap"), String::from("cpu.weight")]);
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.name == "memory.swap.max"));
        assert!(fields.iter().any(|f| f.name == "cpu.weight"));
    }

    #[test]
    fn glob_matches_whole_name() {
        let node = node_with_fields(
            "test",
            vec![
                ("memory.max", "max"),
                ("memory.swap.max", "max"),
                ("memory.swap.max.extra", "x"),
                ("memory.swap.current", "0"),
                ("cpu.max", "100000 100000"),
            ],
        );
        let fields = filter_node_fields(&node, &[String::from("memory*.max")]);
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["memory.max", "memory.swap.max"]);
    }

    #[test]
    fn glob_and_substring_patterns_combine() {
        let node = node_with_fields(
            "test",
            vec![
                ("memory.max", "max"),
                ("memory.swap.max", "max"),
                ("cpu.weight", "100"),
                ("pids.max", "max"),
            ],
        );
        let patterns = [String::from("memory*.max"), String::from("weight")];
        let fields = filter_node_fields(&node, &patterns);
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["memory.max", "memory.swap.max", "cpu.weight"]);
    }

    #[test]
    fn glob_cases() {
        assert!(matches_pattern("memory.max", "*"));
        assert!(matches_pattern("", "*"));
        assert!(matches_pattern("memory.max", "*.max"));
        assert!(!matches_pattern("memory.max.x", "*.max"));
        assert!(matches_pattern("memory.max", "memory.*"));
        assert!(matches_pattern("cpu.max", "cpu.ma?"));
        assert!(!matches_pattern("cpu.max", "cpu.m?"));
        assert!(matches_pattern("io.pressure", "i*o*e"));
        assert!(matches_pattern("memory.swap.max", "*swap*"));
        assert!(!matches_pattern("memory.max", "*swap*"));
        assert!(matches_pattern("memory.swap.max", "m**x"));
    }
}
