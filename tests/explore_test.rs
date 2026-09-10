//! End-to-end tests for the interactive explorer (`cgtree explore`).
//!
//! Every test drives the real `App` with real key events against a real
//! on-disk hierarchy and asserts on the rendered terminal screen.

mod harness;

use harness::Fixture;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

/// A small hierarchy used by most tests:
///
///   <root>/
///   ├── init.scope
///   ├── system.slice/
///   │   ├── cron.service
///   │   └── ssh.service
///   └── user.slice
fn basic() -> Fixture {
    let f = Fixture::new();
    f.add("init.scope", &[("memory.max", "max")]);
    f.add("system.slice", &[("memory.max", "1073741824")]);
    f.add(
        "system.slice/ssh.service",
        &[("memory.max", "536870912"), ("cpu.weight", "100")],
    );
    f.add(
        "system.slice/cron.service",
        &[("memory.max", "268435456"), ("cpu.weight", "50")],
    );
    f.add("user.slice", &[("cpu.weight", "200")]);
    f
}

// ============================================================================
// Initial render
// ============================================================================

#[test]
fn opens_with_root_expanded_and_children_collapsed() {
    let f = basic();
    let tui = f.explore(80, 12);

    assert_eq!(
        tui.tree(),
        vec![
            "<root>/",
            "├── init.scope",
            "├── system.slice/",
            "└── user.slice",
        ],
        "root's children show, but system.slice stays collapsed"
    );
}

#[test]
fn directories_with_children_are_marked_with_a_slash() {
    let f = basic();
    let tui = f.explore(80, 12);

    let tree = tui.tree();
    assert!(
        tree.iter().any(|l| l.ends_with("system.slice/")),
        "a node with children gets a trailing slash: {tree:?}"
    );
    assert!(
        tree.iter().any(|l| l.ends_with("user.slice")),
        "a leaf node has no trailing slash: {tree:?}"
    );
}

#[test]
fn selection_starts_on_the_root_row() {
    let f = basic();
    let tui = f.explore(80, 12);

    assert_eq!(tui.selected(), "<root>/");
    assert_eq!(tui.selected_index(), Some(0));
}

#[test]
fn footer_shows_the_default_status() {
    let f = basic();
    let tui = f.explore(200, 12);

    let footer = tui.footer();
    assert!(footer.contains("props:hide"), "footer: {footer}");
    assert!(footer.contains("f filter"), "footer: {footer}");
    assert!(footer.contains("q/esc quit"), "footer: {footer}");
}

// ============================================================================
// Navigation
// ============================================================================

#[test]
fn j_and_k_move_the_selection_down_and_up() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('j');
    assert_eq!(tui.selected(), "├── init.scope");

    tui.press('j');
    assert_eq!(tui.selected(), "├── system.slice/");

    tui.press('k');
    assert_eq!(tui.selected(), "├── init.scope");
}

#[test]
fn arrow_keys_move_the_selection_like_j_and_k() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.key(KeyCode::Down).key(KeyCode::Down);
    assert_eq!(tui.selected(), "├── system.slice/");

    tui.key(KeyCode::Up);
    assert_eq!(tui.selected(), "├── init.scope");
}

#[test]
fn selection_stops_at_the_top() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('k').press('k').press('k');
    assert_eq!(tui.selected(), "<root>/", "cannot move above the root row");
}

#[test]
fn selection_stops_at_the_bottom() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    for _ in 0..10 {
        tui.press('j');
    }
    assert_eq!(
        tui.selected(),
        "└── user.slice",
        "cannot move past the last row"
    );
}

#[test]
fn g_and_shift_g_jump_to_top_and_bottom() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('G');
    assert_eq!(tui.selected(), "└── user.slice");

    tui.press('g');
    assert_eq!(tui.selected(), "<root>/");
}

#[test]
fn home_and_end_jump_to_top_and_bottom() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.key(KeyCode::End);
    assert_eq!(tui.selected(), "└── user.slice");

    tui.key(KeyCode::Home);
    assert_eq!(tui.selected(), "<root>/");
}

// ============================================================================
// Expanding and collapsing
// ============================================================================

#[test]
fn enter_expands_the_selected_node() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('j').press('j'); // system.slice
    assert_eq!(tui.selected(), "├── system.slice/");

    tui.key(KeyCode::Enter);
    assert_eq!(
        tui.tree(),
        vec![
            "<root>/",
            "├── init.scope",
            "├── system.slice/",
            "│   ├── cron.service",
            "│   └── ssh.service",
            "└── user.slice",
        ],
        "children appear indented under system.slice"
    );
}

#[test]
fn enter_collapses_an_expanded_node() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('j').press('j').key(KeyCode::Enter);
    assert_eq!(tui.tree().len(), 6, "expanded");

    tui.key(KeyCode::Enter);
    assert_eq!(
        tui.tree(),
        vec![
            "<root>/",
            "├── init.scope",
            "├── system.slice/",
            "└── user.slice",
        ],
        "children are hidden again"
    );
}

#[test]
fn space_toggles_expansion_like_enter() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('j').press('j').press(' ');
    assert!(tui.text().contains("ssh.service"), "space expanded the node");

    tui.press(' ');
    assert!(
        !tui.text().contains("ssh.service"),
        "space collapsed the node"
    );
}

#[test]
fn l_expands_but_does_not_collapse() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('j').press('j').press('l');
    assert!(tui.text().contains("ssh.service"), "l expanded");

    tui.press('l');
    assert!(
        tui.text().contains("ssh.service"),
        "a second l leaves the node expanded"
    );
}

#[test]
fn h_collapses_an_expanded_node() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('j').press('j').press('l');
    assert!(tui.text().contains("ssh.service"));

    tui.press('h');
    assert!(!tui.text().contains("ssh.service"), "h collapsed the node");
}

#[test]
fn h_on_a_leaf_jumps_to_its_parent() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    // Expand system.slice and select its first child.
    tui.press('j').press('j').press('l').press('j');
    assert_eq!(tui.selected(), "│   ├── cron.service");

    tui.press('h');
    assert_eq!(
        tui.selected(),
        "├── system.slice/",
        "h on a leaf moves selection up to the parent"
    );
}

#[test]
fn right_and_left_arrows_expand_and_collapse() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('j').press('j').key(KeyCode::Right);
    assert!(tui.text().contains("ssh.service"));

    tui.key(KeyCode::Left);
    assert!(!tui.text().contains("ssh.service"));
}

#[test]
fn expanding_a_leaf_does_nothing() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('j'); // init.scope, a leaf
    let before = tui.tree();
    tui.key(KeyCode::Enter);
    assert_eq!(tui.tree(), before, "a leaf has nothing to expand");
}

#[test]
fn shift_e_expands_every_node() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('E');
    assert_eq!(
        tui.tree(),
        vec![
            "<root>/",
            "├── init.scope",
            "├── system.slice/",
            "│   ├── cron.service",
            "│   └── ssh.service",
            "└── user.slice",
        ]
    );
}

#[test]
fn shift_c_collapses_back_to_the_root() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('E').press('C');
    assert_eq!(
        tui.tree(),
        vec![
            "<root>/",
            "├── init.scope",
            "├── system.slice/",
            "└── user.slice",
        ],
        "the root stays expanded so the tree is never blank"
    );
}

#[test]
fn deep_nesting_renders_stacked_tree_guides() {
    let f = Fixture::new();
    f.add("a", &[]);
    f.add("a/b", &[]);
    f.add("a/b/c", &[]);
    f.add("a/b/c/d", &[]);

    let mut tui = f.explore(80, 12);
    tui.press('E');

    assert_eq!(
        tui.tree(),
        vec![
            "<root>/",
            "└── a/",
            "    └── b/",
            "        └── c/",
            "            └── d",
        ]
    );
}

#[test]
fn siblings_draw_vertical_guides_through_nested_children() {
    let f = Fixture::new();
    f.add("first", &[]);
    f.add("first/child", &[]);
    f.add("second", &[]);

    let mut tui = f.explore(80, 12);
    tui.press('E');

    assert_eq!(
        tui.tree(),
        vec![
            "<root>/",
            "├── first/",
            "│   └── child",
            "└── second",
        ],
        "the guide continues past first's child down to second"
    );
}

// ============================================================================
// Properties display ('p')
// ============================================================================

#[test]
fn p_cycles_from_hidden_to_show_all_and_back() {
    let f = Fixture::new();
    f.add("only", &[("memory.max", "999")]);
    let mut tui = f.explore(200, 20);

    assert!(
        !tui.text().contains("memory.max"),
        "properties are hidden by default"
    );

    tui.press('p');
    assert!(tui.text().contains("memory.max = 999"), "show-all mode");
    assert!(tui.footer().contains("props:show-all"));

    tui.press('p');
    assert!(
        !tui.text().contains("memory.max"),
        "cycles back to hidden with no saved filter"
    );
    assert!(tui.footer().contains("props:hide"));
}

#[test]
fn show_all_lists_properties_under_each_node() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512")]);
    let mut tui = f.explore(80, 24);

    tui.press('E').press('p');

    let text = tui.text();
    assert!(text.contains("cgroup.controllers = cpu memory pids"));
    assert!(text.contains("memory.max = 512"));
}

#[test]
fn properties_are_indented_beneath_their_node() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512")]);
    let mut tui = f.explore(80, 24);

    tui.press('p');

    let lines = tui.tree();
    let node = lines.iter().position(|l| l.contains("svc")).unwrap();
    let prop = lines
        .iter()
        .position(|l| l.contains("memory.max = 512"))
        .unwrap();

    assert!(prop > node, "the property is listed under its node");
    assert!(
        lines[prop].starts_with("    "),
        "the property line is indented: {:?}",
        lines[prop]
    );
}

// ============================================================================
// Property filtering ('f')
// ============================================================================

#[test]
fn f_opens_a_filter_prompt() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('f');
    assert!(
        tui.text().contains("Property filter:"),
        "the prompt is visible: {}",
        tui.text()
    );
}

#[test]
fn typing_a_filter_echoes_it_in_the_prompt() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('f').type_str("memory");
    assert!(
        tui.text().contains("Property filter: memory"),
        "typed text appears: {}",
        tui.text()
    );
}

#[test]
fn applying_a_filter_shows_only_matching_properties() {
    let f = Fixture::new();
    f.add(
        "svc",
        &[("memory.max", "512"), ("cpu.weight", "100"), ("pids.max", "80")],
    );
    let mut tui = f.explore(80, 24);

    tui.press('f').type_str("memory").key(KeyCode::Enter);

    let text = tui.text();
    assert!(text.contains("memory.max = 512"), "matching field shown");
    assert!(!text.contains("cpu.weight"), "non-matching field hidden");
    assert!(!text.contains("pids.max"), "non-matching field hidden");
}

#[test]
fn a_filter_matches_by_substring() {
    let f = Fixture::new();
    f.add(
        "svc",
        &[
            ("memory.swap.max", "0"),
            ("memory.swap.current", "0"),
            ("memory.max", "512"),
        ],
    );
    let mut tui = f.explore(80, 24);

    tui.press('f').type_str("swap").key(KeyCode::Enter);

    let text = tui.text();
    assert!(text.contains("memory.swap.max = 0"));
    assert!(text.contains("memory.swap.current = 0"));
    assert!(
        !text.contains("memory.max = 512"),
        "memory.max does not contain 'swap'"
    );
}

#[test]
fn comma_separated_filters_match_any_pattern() {
    let f = Fixture::new();
    f.add(
        "svc",
        &[("memory.max", "512"), ("cpu.weight", "100"), ("pids.max", "80")],
    );
    let mut tui = f.explore(80, 24);

    tui.press('f').type_str("memory,cpu").key(KeyCode::Enter);

    let text = tui.text();
    assert!(text.contains("memory.max = 512"));
    assert!(text.contains("cpu.weight = 100"));
    assert!(!text.contains("pids.max"), "pids matched neither pattern");
}

#[test]
fn a_star_filter_shows_every_property() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512"), ("cpu.weight", "100")]);
    let mut tui = f.explore(80, 24);

    tui.press('f').type_str("*").key(KeyCode::Enter);

    let text = tui.text();
    assert!(text.contains("memory.max = 512"));
    assert!(text.contains("cpu.weight = 100"));
    assert!(text.contains("cgroup.controllers"));
}

#[test]
fn the_applied_filter_is_shown_in_the_footer() {
    let f = basic();
    let mut tui = f.explore(200, 12);

    tui.press('f').type_str("memory,cpu").key(KeyCode::Enter);

    let footer = tui.footer();
    assert!(footer.contains("f filter:memory,cpu"), "footer: {footer}");
    assert!(footer.contains("props:filtered"), "footer: {footer}");
}

#[test]
fn escape_cancels_the_filter_prompt() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512")]);
    let mut tui = f.explore(80, 20);

    tui.press('f').type_str("memory").key(KeyCode::Esc);

    assert!(
        !tui.text().contains("Property filter:"),
        "the prompt is closed"
    );
    assert!(
        !tui.text().contains("memory.max = 512"),
        "the cancelled filter was not applied"
    );
    assert!(!tui.has_quit(), "esc closed the prompt without quitting");
}

#[test]
fn backspace_edits_the_filter_before_applying() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512"), ("cpu.weight", "100")]);
    let mut tui = f.explore(80, 24);

    tui.press('f')
        .type_str("memoryXX")
        .key(KeyCode::Backspace)
        .key(KeyCode::Backspace)
        .key(KeyCode::Enter);

    let text = tui.text();
    assert!(text.contains("memory.max = 512"), "the corrected filter applied");
    assert!(!text.contains("cpu.weight"));
}

#[test]
fn an_empty_filter_hides_properties() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512")]);
    let mut tui = f.explore(200, 20);

    tui.press('p'); // show all
    assert!(tui.text().contains("memory.max"));

    tui.press('f').key(KeyCode::Enter); // apply an empty filter
    assert!(
        !tui.text().contains("memory.max"),
        "an empty filter falls back to hiding properties"
    );
    assert!(tui.footer().contains("props:hide"));
}

#[test]
fn a_filter_matching_nothing_shows_no_properties() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512")]);
    let mut tui = f.explore(80, 20);

    tui.press('f').type_str("nonexistent").key(KeyCode::Enter);

    assert_eq!(
        tui.tree(),
        vec!["<root>/", "└── svc"],
        "the tree renders normally with no property lines"
    );
}

#[test]
fn p_cycles_through_the_saved_filter_after_one_is_set() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512"), ("cpu.weight", "100")]);
    let mut tui = f.explore(200, 24);

    tui.press('f').type_str("memory").key(KeyCode::Enter);
    assert!(tui.footer().contains("props:filtered"));

    tui.press('p'); // filtered -> hide
    assert!(tui.footer().contains("props:hide"));
    assert!(!tui.text().contains("memory.max"));

    tui.press('p'); // hide -> show all
    assert!(tui.footer().contains("props:show-all"));
    assert!(tui.text().contains("cpu.weight = 100"));

    tui.press('p'); // show all -> filtered, because a filter is saved
    assert!(tui.footer().contains("props:filtered"));
    assert!(tui.text().contains("memory.max = 512"));
    assert!(!tui.text().contains("cpu.weight"));
}

#[test]
fn a_multiline_property_value_renders_across_lines() {
    let f = Fixture::new();
    f.add("svc", &[("memory.stat", "anon 4096\nfile 8192\nkernel 512")]);
    let mut tui = f.explore(80, 24);

    tui.press('f').type_str("memory.stat").key(KeyCode::Enter);

    let text = tui.text();
    assert!(text.contains("memory.stat ="), "the name is on its own line");
    assert!(text.contains("anon 4096"));
    assert!(text.contains("file 8192"));
    assert!(text.contains("kernel 512"));
}

// ============================================================================
// Navigation with property lines on screen
// ============================================================================

#[test]
fn property_lines_are_part_of_the_selection_sequence() {
    let f = Fixture::new();
    f.add("svc", &[("memory.max", "512")]);
    let mut tui = f.explore(80, 24);

    tui.press('f').type_str("memory.max").key(KeyCode::Enter);

    // Root row, then the root's own memory.max is absent (root has one too),
    // so walk down and confirm a property line can hold the selection.
    tui.press('G');
    assert!(
        tui.selected().contains("memory.max = 512"),
        "the last row is a property line: {}",
        tui.selected()
    );
}

#[test]
fn toggling_from_a_property_line_expands_its_parent_node() {
    let f = Fixture::new();
    f.add("parent", &[("memory.max", "512")]);
    f.add("parent/child", &[("memory.max", "256")]);
    let mut tui = f.explore(80, 24);

    tui.press('f').type_str("memory.max").key(KeyCode::Enter);

    // Select the property line belonging to `parent`.
    tui.press('G');
    assert!(tui.selected().contains("memory.max = 512"));

    tui.key(KeyCode::Enter);
    assert!(
        tui.text().contains("child"),
        "toggling on a property line expanded its owning node: {}",
        tui.text()
    );
}

// ============================================================================
// Help screen ('?')
// ============================================================================

#[test]
fn question_mark_opens_the_help_screen() {
    let f = basic();
    let mut tui = f.explore(80, 40);

    tui.press('?');

    let text = tui.text();
    assert!(text.contains("Interactive cgroup hierarchy explorer"));
    assert!(text.contains("Navigation"));
    assert!(text.contains("Expand all nodes"));
    assert!(text.contains("Press ? or Esc to close this help"));
}

#[test]
fn help_replaces_the_tree_while_open() {
    let f = basic();
    let mut tui = f.explore(80, 40);

    tui.press('?');
    assert!(
        !tui.text().contains("system.slice"),
        "the tree is not drawn behind the help screen"
    );
}

#[test]
fn question_mark_closes_the_help_screen() {
    let f = basic();
    let mut tui = f.explore(80, 40);

    tui.press('?').press('?');
    assert!(tui.text().contains("system.slice"), "back to the tree");
    assert!(!tui.has_quit());
}

#[test]
fn escape_closes_help_without_quitting() {
    let f = basic();
    let mut tui = f.explore(80, 40);

    tui.press('?').key(KeyCode::Esc);
    assert!(tui.text().contains("system.slice"), "back to the tree");
    assert!(
        !tui.has_quit(),
        "esc closes help rather than exiting the app"
    );
}

#[test]
fn q_closes_help_without_quitting() {
    let f = basic();
    let mut tui = f.explore(80, 40);

    tui.press('?').press('q');
    assert!(!tui.has_quit(), "q closes help rather than exiting the app");
    assert!(tui.text().contains("system.slice"));
}

#[test]
fn navigation_keys_are_inert_while_help_is_open() {
    let f = basic();
    let mut tui = f.explore(80, 40);

    tui.press('j'); // select init.scope
    let before = tui.selected();

    tui.press('?').press('j').press('j').press('?');
    assert_eq!(
        tui.selected(),
        before,
        "j did not move the selection while help was open"
    );
}

#[test]
fn help_reflects_the_current_props_and_filter_state() {
    let f = basic();
    let mut tui = f.explore(80, 40);

    tui.press('f').type_str("memory").key(KeyCode::Enter);
    tui.press('?');

    let text = tui.text();
    assert!(text.contains("(mode:filtered)"), "help shows props mode");
    assert!(text.contains("(filter:memory)"), "help shows the filter");
}

// ============================================================================
// Quitting
// ============================================================================

#[test]
fn q_quits() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('q');
    assert!(tui.has_quit());
}

#[test]
fn escape_quits_from_the_tree() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.key(KeyCode::Esc);
    assert!(tui.has_quit());
}

#[test]
fn ctrl_c_quits() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.key_with(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(tui.has_quit());
}

#[test]
fn ctrl_c_quits_even_from_the_filter_prompt() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('f');
    tui.key_with(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(
        tui.has_quit(),
        "ctrl-c is handled before the filter input sees it"
    );
}

#[test]
fn ctrl_c_quits_from_the_help_screen() {
    let f = basic();
    let mut tui = f.explore(80, 40);

    tui.press('?');
    tui.key_with(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(tui.has_quit());
}

#[test]
fn q_typed_into_the_filter_prompt_does_not_quit() {
    let f = basic();
    let mut tui = f.explore(80, 12);

    tui.press('f').press('q');
    assert!(!tui.has_quit(), "q is text while the prompt is open");
    assert!(tui.text().contains("Property filter: q"));
}

// ============================================================================
// Refresh ('r')
// ============================================================================

#[test]
fn r_picks_up_cgroups_created_since_launch() {
    let f = basic();
    let mut tui = f.explore(80, 16);

    assert!(!tui.text().contains("late.service"));

    f.add("late.service", &[]);
    tui.press('r');

    assert!(
        tui.text().contains("late.service"),
        "refresh rescanned the hierarchy: {}",
        tui.text()
    );
}

#[test]
fn r_drops_cgroups_removed_since_launch() {
    let f = basic();
    let mut tui = f.explore(80, 16);
    assert!(tui.text().contains("init.scope"));

    std::fs::remove_dir_all(f.root().join("init.scope")).unwrap();
    tui.press('r');

    assert!(
        !tui.text().contains("init.scope"),
        "refresh dropped the removed cgroup"
    );
}

#[test]
fn r_picks_up_changed_property_values() {
    let f = Fixture::new();
    f.add("svc", &[("memory.current", "1000")]);
    let mut tui = f.explore(80, 20);

    tui.press('f').type_str("memory.current").key(KeyCode::Enter);
    assert!(tui.text().contains("memory.current = 1000"));

    std::fs::write(f.root().join("svc/memory.current"), "2000\n").unwrap();
    tui.press('r');

    assert!(
        tui.text().contains("memory.current = 2000"),
        "refresh re-read the property: {}",
        tui.text()
    );
}

#[test]
fn r_keeps_the_selection_in_range_when_the_tree_shrinks() {
    let f = basic();
    let mut tui = f.explore(80, 16);

    tui.press('E').press('G');
    let last = tui.selected();
    assert!(!last.is_empty());

    // Remove most of the tree, then refresh.
    std::fs::remove_dir_all(f.root().join("system.slice")).unwrap();
    std::fs::remove_dir_all(f.root().join("user.slice")).unwrap();
    tui.press('r');

    assert!(
        tui.selected_index().is_some(),
        "a row is still highlighted after the tree shrank"
    );
    assert_eq!(tui.tree(), vec!["<root>/", "└── init.scope"]);
}

#[test]
fn r_preserves_expansion_state() {
    let f = basic();
    let mut tui = f.explore(80, 16);

    tui.press('E');
    assert!(tui.text().contains("ssh.service"));

    tui.press('r');
    assert!(
        tui.text().contains("ssh.service"),
        "expanded nodes stay expanded across a refresh"
    );
}

// ============================================================================
// Edge cases
// ============================================================================

#[test]
fn a_hierarchy_with_only_a_root_renders() {
    let f = Fixture::new();
    let tui = f.explore(80, 10);

    assert_eq!(
        tui.tree(),
        vec!["<root>"],
        "a childless root is a leaf, so it gets no trailing slash"
    );
    assert_eq!(tui.selected(), "<root>");
}

#[test]
fn navigation_is_safe_on_a_root_only_hierarchy() {
    let f = Fixture::new();
    let mut tui = f.explore(80, 10);

    tui.press('j').press('j').press('k').press('G').press('g');
    tui.key(KeyCode::Enter).press('E').press('C');

    assert_eq!(tui.selected(), "<root>", "still on the only row");
    assert!(!tui.has_quit());
}

#[test]
fn unknown_keys_are_ignored() {
    let f = basic();
    let mut tui = f.explore(80, 12);
    let before = tui.tree();

    tui.press('z').press('X').press('7');

    assert_eq!(tui.tree(), before, "the screen is unchanged");
    assert!(!tui.has_quit());
}

#[test]
fn many_siblings_all_render() {
    let f = Fixture::new();
    for i in 0..20 {
        f.add(&format!("svc-{i:02}"), &[]);
    }
    let tui = f.explore(80, 30);

    let tree = tui.tree();
    assert_eq!(tree.len(), 21, "root plus 20 children");
    assert_eq!(tree[1], "├── svc-00");
    assert_eq!(tree[20], "└── svc-19", "the last child gets the elbow");
}

#[test]
fn children_are_listed_in_sorted_order() {
    let f = Fixture::new();
    f.add("zebra", &[]);
    f.add("alpha", &[]);
    f.add("middle", &[]);

    let tui = f.explore(80, 12);
    assert_eq!(
        tui.tree(),
        vec!["<root>/", "├── alpha", "├── middle", "└── zebra"]
    );
}

#[test]
fn a_narrow_terminal_still_renders_the_tree() {
    let f = basic();
    let tui = f.explore(24, 10);

    let tree = tui.tree();
    assert!(tree[0].starts_with('/') || tree[0].contains("<root>"));
    assert!(
        tree.iter().any(|l| l.contains("system.slice")),
        "rows are present even when truncated: {tree:?}"
    );
}

#[test]
fn a_tree_taller_than_the_terminal_does_not_panic() {
    let f = Fixture::new();
    for i in 0..50 {
        f.add(&format!("svc-{i:02}"), &[]);
    }
    let mut tui = f.explore(80, 8);

    tui.press('G');
    assert!(!tui.has_quit());
    assert!(!tui.text().is_empty(), "something is on screen");
}

#[test]
fn an_empty_property_value_renders_the_name_with_no_value() {
    let f = Fixture::new();
    let dir = f.root().join("svc");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("cgroup.controllers"), "cpu\n").unwrap();
    std::fs::write(dir.join("cgroup.events"), "").unwrap();

    let mut tui = f.explore(80, 20);
    tui.press('f').type_str("cgroup.events").key(KeyCode::Enter);

    assert!(
        tui.text().contains("cgroup.events ="),
        "the empty field is listed: {}",
        tui.text()
    );
}
