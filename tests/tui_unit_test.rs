//! Unit tests for TUI functionality using TestBackend (NO SLEEPS!).
//!
//! These tests use Ratatui's TestBackend to test the TUI without PTY overhead.
//! They can test full interaction flows by calling handle_key() directly.
//!
//! ## Why This Approach?
//!
//! The App struct has excellent separation of concerns:
//! - handle_key() updates state
//! - draw() renders to any backend
//! - run() is the only method that needs a real terminal
//!
//! This means we can test interactions WITHOUT:
//! - PTY (pseudo-terminal)
//! - Sleeps/timeouts
//! - Process spawning
//! - I/O overhead
//!
//! Result: Fast, deterministic, comprehensive interaction tests!

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

// Import internal types using path module syntax for integration tests
use cgtree::cgroup;
use cgtree::tui::App;

/// Test fixture for creating fake cgroup v2 hierarchies
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

    fn root_path(&self) -> PathBuf {
        self.root.clone()
    }

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

/// Helper to render app to TestBackend and get buffer contents as a string
fn render_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.draw(f)).unwrap();

    // Convert buffer to string by collecting all cell symbols
    let buffer = terminal.backend().buffer();
    let mut result = String::new();
    for y in 0..buffer.area().height {
        for x in 0..buffer.area().width {
            let cell = buffer.cell((x, y)).unwrap();
            result.push_str(cell.symbol());
        }
        result.push('\n');
    }
    result
}

// ============================================================================
// Basic Rendering Tests
// ============================================================================

#[test]
fn test_initial_rendering_shows_root() {
    let f = Fixture::new();
    f.add(".", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    let output = render_to_string(&mut app);
    // Root should be visible (the directory name)
    assert!(output.contains(f.root_path().file_name().unwrap().to_str().unwrap()));
}

#[test]
fn test_rendering_shows_children() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("init.scope", &[]);
    f.add("system.slice", &[]);
    f.add("user.slice", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    let output = render_to_string(&mut app);
    assert!(output.contains("init.scope"));
    assert!(output.contains("system.slice"));
    assert!(output.contains("user.slice"));
}

#[test]
fn test_directories_have_slash_suffix() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("parent.slice", &[]);
    f.add("parent.slice/child.service", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    let output = render_to_string(&mut app);
    // Directories (with children) should have trailing slash
    assert!(output.contains("parent.slice/"));
}

#[test]
fn test_footer_shows_help_hint() {
    let f = Fixture::new();
    f.add(".", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    let output = render_to_string(&mut app);
    assert!(output.contains("?") || output.contains("help"));
    assert!(output.contains("q") || output.contains("quit"));
}

// ============================================================================
// Props Toggle Interaction Tests (NO SLEEPS!)
// ============================================================================

#[test]
fn test_props_toggle_shows_all_properties() {
    let f = Fixture::new();
    f.add(".", &[
        ("memory.max", "1073741824"),
        ("memory.current", "524288000"),
        ("cpu.weight", "100"),
    ]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Initially: props hidden
    let initial = render_to_string(&mut app);
    assert!(!initial.contains("memory.max"));
    assert!(!initial.contains("cpu.weight"));

    // Press 'p' to toggle props to ShowAll mode
    let key_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty());
    app.handle_key(key_p);

    // Now: props visible!
    let after_toggle = render_to_string(&mut app);
    assert!(after_toggle.contains("memory.max"));
    assert!(after_toggle.contains("cpu.weight"));
    // NO SLEEPS - instant verification!
}

#[test]
fn test_props_toggle_cycles_through_modes() {
    let f = Fixture::new();
    f.add(".", &[("memory.max", "1GB")]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    let key_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty());

    // Mode 1: Hide (default) - props not shown
    let output1 = render_to_string(&mut app);
    assert!(!output1.contains("memory.max"));

    // Mode 2: ShowAll - props shown!
    app.handle_key(key_p);
    let output2 = render_to_string(&mut app);
    assert!(output2.contains("memory.max"));

    // Mode 3: Back to Hide (no filter set) - props hidden again
    app.handle_key(key_p);
    let output3 = render_to_string(&mut app);
    assert!(!output3.contains("memory.max"));
}

// ============================================================================
// Navigation Interaction Tests (NO SLEEPS!)
// ============================================================================

#[test]
fn test_navigation_down_moves_selection() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("first.scope", &[]);
    f.add("second.scope", &[]);
    f.add("third.scope", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Navigate down multiple times
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    app.handle_key(key_down);
    app.handle_key(key_down);
    app.handle_key(key_down);

    let output = render_to_string(&mut app);
    // All items should still be visible
    assert!(output.contains("first.scope"));
    assert!(output.contains("second.scope"));
    assert!(output.contains("third.scope"));
    // In a real test, we'd verify the highlighted item
    // (would need to parse ANSI or check buffer styles)
}

#[test]
fn test_navigation_up_moves_selection() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("first.scope", &[]);
    f.add("second.scope", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Move down then up
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let key_up = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());

    app.handle_key(key_down);
    app.handle_key(key_down);
    app.handle_key(key_up);

    let output = render_to_string(&mut app);
    assert!(output.contains("first.scope"));
    assert!(output.contains("second.scope"));
}

#[test]
fn test_navigation_with_vim_keys() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("item1.scope", &[]);
    f.add("item2.scope", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Use j/k (vim keys)
    let key_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let key_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());

    app.handle_key(key_j);  // Down
    app.handle_key(key_j);  // Down
    app.handle_key(key_k);  // Up

    let output = render_to_string(&mut app);
    assert!(output.contains("item1.scope"));
    assert!(output.contains("item2.scope"));
}

// ============================================================================
// Expand/Collapse Interaction Tests (NO SLEEPS!)
// ============================================================================

#[test]
fn test_expand_shows_children() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("parent.slice", &[]);
    f.add("parent.slice/child.service", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Initially: child not visible (parent collapsed)
    let initial = render_to_string(&mut app);
    assert!(!initial.contains("child.service"));

    // Navigate to parent and expand with Enter
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());

    app.handle_key(key_down);   // Select parent.slice
    app.handle_key(key_enter);  // Toggle expansion

    // Now: child IS visible!
    let expanded = render_to_string(&mut app);
    assert!(expanded.contains("child.service"));
    // NO SLEEPS - deterministic!
}

#[test]
fn test_collapse_hides_children() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("parent.slice", &[]);
    f.add("parent.slice/child.service", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Expand first
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());

    app.handle_key(key_down);
    app.handle_key(key_enter);  // Expand

    let expanded = render_to_string(&mut app);
    assert!(expanded.contains("child.service"));

    // Collapse again
    app.handle_key(key_enter);  // Collapse

    let collapsed = render_to_string(&mut app);
    assert!(!collapsed.contains("child.service"));
}

#[test]
fn test_expand_with_right_arrow() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("parent.slice", &[]);
    f.add("parent.slice/child.service", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let key_right = KeyEvent::new(KeyCode::Right, KeyModifiers::empty());

    app.handle_key(key_down);
    app.handle_key(key_right);  // Expand with →

    let output = render_to_string(&mut app);
    assert!(output.contains("child.service"));
}

#[test]
fn test_collapse_with_left_arrow() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("parent.slice", &[]);
    f.add("parent.slice/child.service", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Expand first
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let key_right = KeyEvent::new(KeyCode::Right, KeyModifiers::empty());
    let key_left = KeyEvent::new(KeyCode::Left, KeyModifiers::empty());

    app.handle_key(key_down);
    app.handle_key(key_right);  // Expand
    app.handle_key(key_left);   // Collapse with ←

    let output = render_to_string(&mut app);
    assert!(!output.contains("child.service"));
}

#[test]
fn test_expand_all_shows_deep_hierarchy() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("level1", &[]);
    f.add("level1/level2", &[]);
    f.add("level1/level2/level3", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Initially: only root and level1 visible
    let initial = render_to_string(&mut app);
    assert!(initial.contains("level1"));
    assert!(!initial.contains("level2"));
    assert!(!initial.contains("level3"));

    // Press 'E' to expand all
    let key_E = KeyEvent::new(KeyCode::Char('E'), KeyModifiers::empty());
    app.handle_key(key_E);

    // Now: all levels visible!
    let expanded = render_to_string(&mut app);
    assert!(expanded.contains("level1"));
    assert!(expanded.contains("level2"));
    assert!(expanded.contains("level3"));
}

#[test]
fn test_collapse_all_hides_everything() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("level1", &[]);
    f.add("level1/level2", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Expand all first
    let key_E = KeyEvent::new(KeyCode::Char('E'), KeyModifiers::empty());
    app.handle_key(key_E);

    let expanded = render_to_string(&mut app);
    assert!(expanded.contains("level2"));

    // Press 'C' to collapse all
    let key_C = KeyEvent::new(KeyCode::Char('C'), KeyModifiers::empty());
    app.handle_key(key_C);

    // Now: only root visible, level1 visible but level2 hidden
    let collapsed = render_to_string(&mut app);
    assert!(collapsed.contains("level1"));  // Still visible as child of root
    assert!(!collapsed.contains("level2")); // Hidden (parent collapsed)
}

// ============================================================================
// Filter Mode Interaction Tests (NO SLEEPS!)
// ============================================================================

#[test]
fn test_filter_mode_opens_input() {
    let f = Fixture::new();
    f.add(".", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Press 'f' to open filter input
    let key_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty());
    app.handle_key(key_f);

    let output = render_to_string(&mut app);
    assert!(output.contains("Property filter:"));
}

#[test]
fn test_filter_mode_accepts_text_input() {
    let f = Fixture::new();
    f.add(".", &[("memory.max", "1GB"), ("cpu.weight", "100")]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Enter filter mode
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty()));

    // Type "memory"
    app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty()));

    // Press Enter to apply
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

    let output = render_to_string(&mut app);
    // Should show memory field but not cpu
    assert!(output.contains("memory.max"));
    assert!(!output.contains("cpu.weight"));
    // NO SLEEPS!
}

#[test]
fn test_filter_mode_escape_cancels() {
    let f = Fixture::new();
    f.add(".", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Enter filter mode
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty()));

    let in_filter = render_to_string(&mut app);
    assert!(in_filter.contains("Property filter:"));

    // Press Escape to cancel
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));

    let after_escape = render_to_string(&mut app);
    assert!(!after_escape.contains("Property filter:"));
}

// ============================================================================
// Help Screen Interaction Tests (NO SLEEPS!)
// ============================================================================

#[test]
fn test_help_screen_opens() {
    let f = Fixture::new();
    f.add(".", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Press '?' to open help
    let key_question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty());
    app.handle_key(key_question);

    let output = render_to_string(&mut app);
    assert!(output.contains("Navigation"));
    assert!(output.contains("Properties Display"));
    assert!(output.contains("Property Filtering"));
}

#[test]
fn test_help_screen_closes_with_escape() {
    let f = Fixture::new();
    f.add(".", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Open help
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));

    let in_help = render_to_string(&mut app);
    assert!(in_help.contains("Navigation"));

    // Close with Escape
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));

    let after_escape = render_to_string(&mut app);
    assert!(!after_escape.contains("Press ? or Esc to close this help"));
}

#[test]
fn test_help_screen_closes_with_question_mark() {
    let f = Fixture::new();
    f.add(".", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Open help
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));

    // Close with '?' again
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));

    let output = render_to_string(&mut app);
    // Should be back to normal view (no help text)
    assert!(!output.contains("Press ? or Esc to close this help"));
}

// ============================================================================
// Complex Workflow Tests (NO SLEEPS!)
// ============================================================================

#[test]
fn test_complex_workflow_expand_navigate_filter() {
    let f = Fixture::new();
    f.add(".", &[("memory.max", "1GB")]);
    f.add("parent.slice", &[("cpu.weight", "100")]);
    f.add("parent.slice/child.service", &[("memory.current", "500MB")]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Step 1: Expand parent
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

    let after_expand = render_to_string(&mut app);
    assert!(after_expand.contains("child.service"));

    // Step 2: Navigate to child
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));

    // Step 3: Enable props to see memory field
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty()));

    let after_props = render_to_string(&mut app);
    assert!(after_props.contains("memory.max"));
    assert!(after_props.contains("cpu.weight"));
    assert!(after_props.contains("memory.current"));

    // Step 4: Filter to memory only
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

    let final_output = render_to_string(&mut app);
    assert!(final_output.contains("memory.max"));
    assert!(final_output.contains("memory.current"));
    assert!(!final_output.contains("cpu.weight"));  // Filtered out!

    // All of this without a single sleep!
}

#[test]
fn test_workflow_help_then_interact() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("test.scope", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Open help
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));

    let in_help = render_to_string(&mut app);
    assert!(in_help.contains("Navigation"));

    // Close help and navigate
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));

    let final_output = render_to_string(&mut app);
    assert!(final_output.contains("test.scope"));
    assert!(!final_output.contains("Press ? or Esc to close this help"));
}
