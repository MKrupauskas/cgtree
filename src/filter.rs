use crate::data::FieldEntry;

/// Parses a comma-separated string into field name patterns.
/// Example: `memory,cpu.weight` becomes `["memory", "cpu.weight"]`.
pub fn parse_field_patterns(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Selects the fields matching the given patterns.
///
/// - no patterns selects nothing (properties are opt-in)
/// - `*` selects every field
/// - otherwise a field matches when its name contains any pattern as a substring
pub fn matching_fields<'a>(
    fields: &'a [FieldEntry],
    patterns: &'a [String],
) -> impl Iterator<Item = &'a FieldEntry> {
    let show_all = patterns.iter().any(|p| p == "*");
    fields
        .iter()
        .filter(move |field| show_all || patterns.iter().any(|p| field.name.contains(p)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(entries: Vec<(&str, &str)>) -> Vec<FieldEntry> {
        entries
            .into_iter()
            .map(|(name, value)| FieldEntry {
                name: name.to_string(),
                value: value.to_string(),
            })
            .collect()
    }

    fn matched(entries: Vec<(&str, &str)>, patterns: &[&str]) -> Vec<String> {
        let fields = fields(entries);
        let patterns: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();
        matching_fields(&fields, &patterns)
            .map(|f| f.name.clone())
            .collect()
    }

    #[test]
    fn parse_empty_string() {
        assert_eq!(parse_field_patterns(""), Vec::<String>::new());
    }

    #[test]
    fn parse_single_pattern() {
        assert_eq!(parse_field_patterns("memory"), vec!["memory"]);
    }

    #[test]
    fn parse_multiple_patterns() {
        assert_eq!(
            parse_field_patterns("memory,cpu,pids"),
            vec!["memory", "cpu", "pids"]
        );
    }

    #[test]
    fn parse_with_whitespace() {
        assert_eq!(
            parse_field_patterns("memory, cpu.weight , pids"),
            vec!["memory", "cpu.weight", "pids"]
        );
    }

    #[test]
    fn no_patterns_matches_nothing() {
        assert!(matched(vec![("memory.max", "max")], &[]).is_empty());
    }

    #[test]
    fn star_matches_everything() {
        let names = matched(
            vec![
                ("memory.max", "max"),
                ("cpu.weight", "100"),
                ("pids.max", "max"),
            ],
            &["*"],
        );
        assert_eq!(names, ["memory.max", "cpu.weight", "pids.max"]);
    }

    #[test]
    fn matches_by_substring() {
        let names = matched(
            vec![
                ("memory.swap.max", "max"),
                ("memory.swap.current", "0"),
                ("memory.max", "1073741824"),
                ("cpu.weight", "100"),
            ],
            &["swap"],
        );
        assert_eq!(names, ["memory.swap.max", "memory.swap.current"]);
    }

    #[test]
    fn matches_any_of_several_patterns() {
        let names = matched(
            vec![
                ("memory.swap.max", "max"),
                ("memory.max", "1073741824"),
                ("cpu.weight", "100"),
                ("cpu.max", "100000 100000"),
            ],
            &["swap", "cpu.weight"],
        );
        assert_eq!(names, ["memory.swap.max", "cpu.weight"]);
    }
}
