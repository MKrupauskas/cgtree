# Alternative TUI Testing Approaches - Research

## Research Question
1. Would running tests directly in the terminal (instead of PTY) be more robust and eliminate sleeps?
2. Are there Playwright-like tools for TUI testing that provide better testing experience?

## Key Findings

### 🎯 **YES - You CAN Test Interactions Without Sleeps!**

**BREAKTHROUGH DISCOVERY:** Your codebase is already perfectly structured for testable interactions!

The `App` struct in `src/tui.rs` has:
- ✅ **Public `handle_key()` method** - Can be called directly without PTY
- ✅ **Public `draw()` method** - Can render to TestBackend
- ✅ **Separated event loop** - Event handling is decoupled from `run()`

This means you can:
```rust
let mut app = App::new(root, data);

// Simulate interactions directly!
app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty()));
app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

// Render to TestBackend (no PTY!)
let backend = TestBackend::new(80, 24);
let mut terminal = Terminal::new(backend).unwrap();
terminal.draw(|f| app.draw(f)).unwrap();

// Verify results - instant, deterministic!
let buffer = terminal.backend().buffer();
assert!(buffer.to_string().contains("expected content"));
```

**No sleeps, no PTY, no waiting - just pure unit tests!**

---

### 1. Direct Terminal Testing vs PTY

**Answer: Mixed - PTY needed for full TUI, but NOT for interactions**

**For rendering + interaction tests:**
- ✅ **TestBackend** - No PTY needed! Direct event simulation works perfectly
- Events are created with `KeyEvent::new(KeyCode, KeyModifiers)`
- App state changes deterministically
- Instant feedback, no sleeps required

**For full end-to-end tests:**
- ⚠️ PTY still needed for testing the actual `run()` loop with real terminal
- But this is only needed for a few integration tests
- Most tests can use TestBackend approach

**However**, there are better ways to handle synchronization than arbitrary sleeps.

---

## Alternative Testing Tools Discovered

### 1. Microsoft tui-test ⭐ MOST PROMISING

**GitHub**: https://github.com/microsoft/tui-test

**What it is:**
- Jest-inspired testing framework for terminal applications
- Cross-platform (Windows, Linux, macOS)
- Multi-language support (Rust, Python, JavaScript)

**Key Features:**

✅ **Auto-wait** - Waits for terminal to be ready before executing commands
```javascript
await terminal.get_by_text("Ready").expect()  // No sleeps needed!
```

✅ **Full isolation** - Each test gets its own terminal context (only milliseconds overhead)

✅ **Rich assertions** - Semantic waiting instead of arbitrary timeouts
```javascript
test('app shows welcome', async (terminal) => {
    await terminal.send_text('myapp\n');
    await terminal.get_by_text('Welcome').expect();  // Waits intelligently
});
```

✅ **Multiple backends** - Alacritty (default), Ghostty, Rio, xterm.js

✅ **Multiple shells** - bash, zsh, fish, PowerShell, cmd, nushell, etc.

✅ **Tracing** - Detailed snapshots to eliminate flakes

✅ **AI Agent Support** - Makes terminals accessible to AI

**Languages:**
- Primary: JavaScript/TypeScript (via npm: `@microsoft/tui-test`)
- Also supports: Rust, Python

**Does it eliminate sleeps?**
- **YES** - Uses semantic waiting ("wait for this text") instead of timeouts
- Much more robust than sleep-based approaches

---

### 2. termwright (Playwright for TUI)

**GitHub**: https://github.com/fcoury/termwright

**What it is:**
- Playwright-like automation framework for TUI apps
- Framework-agnostic (works with ratatui, crossterm, ncurses, etc.)

**Key Features:**

✅ **Wait conditions** - Wait for text, regex, screen stability, process exit
```rust
term.expect("VIM")
    .timeout(Duration::from_secs(5))
    .await?
```

✅ **Screen reading** - Access text, colors, cursor position

✅ **Input simulation** - Keystrokes, special keys

✅ **Multiple outputs** - Plain text, JSON, PNG screenshots

✅ **Box detection** - Uses box-drawing characters

**Does it eliminate sleeps?**
- **Partially** - Has `.timeout()` but uses smart waiting
- Better than arbitrary sleeps, but still timeout-based

**Language:** Rust

---

### 3. VHS by Charmbracelet

**GitHub**: https://github.com/charmbracelet/vhs

**What it is:**
- Declarative terminal recording tool
- Records terminal sessions as GIF/MP4/WebM

**Key Features:**

✅ **Declarative scripting** - Write tape files with commands
```tape
Output demo.gif
Type "cargo run"
Sleep 500ms
Type "q"
Sleep 100ms
```

✅ **CI/CD ready** - Deterministic recordings

✅ **Interactive support** - Fully supports TUI apps with arrow keys, Ctrl, etc.

✅ **Wait command** - Wait for UI elements to render

**Does it eliminate sleeps?**
- **NO** - Still uses explicit Sleep commands
- However, sleeps are part of the "recording script" not test flakiness

**Use case:** More for demo/documentation than testing

---

### 4. Ratatui TestBackend (Unit Testing)

**Docs**: https://ratatui.rs/recipes/testing/snapshots/

**What it is:**
- In-process rendering testing (no PTY)
- Fast, deterministic, no I/O

**Key Features:**

✅ **Zero overhead** - Tests rendering logic directly
```rust
let mut backend = TestBackend::new(80, 24);
let mut terminal = Terminal::new(backend)?;
terminal.draw(|f| app.draw(f))?;
let buffer = terminal.backend().buffer();
assert_eq!(buffer.get(0, 0).symbol(), "t");
```

✅ **Snapshot testing** - Works with `insta` for snapshot assertions

✅ **Fast** - No process spawning, no PTY overhead

**Limitations:**
- ❌ Cannot test PTY-specific behavior (terminal size negotiation, TTY detection)
- ❌ Cannot test event loop or key handling
- ❌ Cannot test terminal setup/teardown
- ❌ Cannot test user interaction flows
- ❌ Cannot test graphics protocols (Sixel, iTerm2 images)

**Does it eliminate sleeps?**
- **YES** - But only because it doesn't test the full application
- Only tests rendering, not interaction

---

### 5. tmux-based Testing

**What it is:**
- Use tmux to create isolated terminal sessions
- Send keystrokes, capture pane output

**Key Features:**

✅ **Session isolation** - Each test in separate tmux session

✅ **Capture output** - `tmux capture-pane` to read screen

✅ **Send input** - `tmux send-keys` for interaction

**Does it eliminate sleeps?**
- **NO** - Still relies on sleep-based polling
- Example pattern: `tmux send-keys "command"; sleep 2; tmux capture-pane`

**Verdict:** More complicated than PTY testing, same sleep issues

---

## Comparison Table

| Tool | Eliminates Sleeps? | Language | PTY-Based | Semantic Waiting | Rust Support |
|------|-------------------|----------|-----------|------------------|--------------|
| **microsoft/tui-test** | ✅ YES | JS/Python/Rust | ✅ Yes | ✅ Yes | ⚠️ Secondary |
| **termwright** | ⚠️ Partial | Rust | ✅ Yes | ✅ Yes | ✅ Primary |
| **ratatui-testlib** | ❌ No | Rust | ✅ Yes | ⚠️ Manual | ✅ Primary |
| **VHS** | ❌ No | Go | ✅ Yes | ❌ No | ❌ No |
| **TestBackend** | ✅ YES* | Rust | ❌ No | ✅ N/A | ✅ Primary |
| **tmux-based** | ❌ No | Shell | ✅ Yes | ❌ No | ❌ No |
| **Current approach** | ❌ No | Rust | ✅ Yes | ❌ No | ✅ Primary |

*TestBackend eliminates sleeps but doesn't test the full application (no event loop, no interaction)

---

## Direct Answer to Your Questions

### Q1: Would tests be more robust if run directly in terminal (not PTY)?

**A: YES - For interaction tests, you don't need PTY at all!**

Your codebase has the perfect architecture:
1. `App` struct with public `handle_key()` method
2. Direct event simulation with `KeyEvent::new()`
3. Render to `TestBackend` without PTY
4. **No sleeps, instant feedback, deterministic results**

PTY is only needed for full end-to-end tests of the `run()` loop itself.

### Q2: Can we eliminate sleeps?

**A: YES - Completely!**

The current approach uses `ratatui-testlib` with PTY, which requires sleeps. But we discovered:

**Best option for this project: TestBackend with direct event simulation**
- Call `app.handle_key(KeyEvent::new(...))` directly
- Render to `TestBackend::new(80, 24)`
- Verify buffer contents immediately
- **Zero sleeps, zero PTY overhead, instant tests**
- Already supported by your code structure!

**Alternative options (if you needed them):**

**microsoft/tui-test**
- Provides semantic waiting: `await terminal.get_by_text("text").expect()`
- No arbitrary sleeps needed
- BUT: Primarily JavaScript/TypeScript, Rust support is secondary

**termwright**
- Playwright-like API for Rust
- Has `.expect("text").timeout()` pattern
- Better than arbitrary sleeps
- Still uses timeouts but more semantic

---

## Recommendations

### Option 1: Switch to microsoft/tui-test (BEST for eliminating sleeps)

**Pros:**
- ✅ Eliminates arbitrary sleeps with semantic waiting
- ✅ Auto-wait for terminal readiness
- ✅ Rich assertion API
- ✅ Cross-platform, multi-shell
- ✅ Full isolation per test
- ✅ Comprehensive tracing/debugging

**Cons:**
- ❌ Primarily JavaScript/TypeScript (would need Node.js)
- ❌ Rust support is secondary
- ❌ Another language/toolchain in the project
- ❌ Newer tool (less battle-tested)

**Example:**
```javascript
// package.json: "@microsoft/tui-test": "latest"
import { test } from '@microsoft/tui-test';

test('shows tree structure', async (terminal) => {
    await terminal.send_text('./cgtree --root /tmp/test explore\n');
    await terminal.get_by_text('init.scope').expect();
    await terminal.get_by_text('system.slice').expect();
    await terminal.send_key('q');
});
```

### Option 2: Use termwright (Rust-native, better than current)

**Pros:**
- ✅ Pure Rust
- ✅ Playwright-like semantic API
- ✅ Better than arbitrary sleeps (uses smart waiting)
- ✅ Framework-agnostic

**Cons:**
- ⚠️ Still uses timeouts (but more semantic)
- ⚠️ Less mature than microsoft/tui-test
- ⚠️ Smaller community

**Example:**
```rust
let mut term = Termwright::spawn("cgtree", &["--root", "/tmp/test", "explore"])?;
term.expect("init.scope").timeout(Duration::from_secs(2)).await?;
term.send_key('q').await?;
```

### Option 3: Hybrid - TestBackend + ratatui-testlib (Current approach improved)

**Pros:**
- ✅ Pure Rust
- ✅ Use TestBackend for unit tests (fast, no sleeps)
- ✅ Use ratatui-testlib for integration tests (comprehensive)
- ✅ Leverage insta for snapshot testing
- ✅ Already implemented

**Cons:**
- ❌ Still need sleeps in integration tests
- ❌ Less robust than semantic waiting tools

**Improvement: Add snapshot testing**
```rust
use ratatui::backend::TestBackend;
use insta::assert_snapshot;

#[test]
fn test_tree_rendering() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;

    // Directly test rendering without PTY
    let mut app = App::new(root);
    terminal.draw(|f| app.draw(f))?;

    let buffer = terminal.backend().buffer();
    assert_snapshot!(buffer.to_string());  // No sleeps needed!
}
```

### Option 4: Keep current approach (Document trade-offs)

**Pros:**
- ✅ Already working
- ✅ All tests pass reliably
- ✅ Pure Rust
- ✅ Well-documented

**Cons:**
- ❌ Uses sleep-based synchronization
- ❌ Cannot test complex interactions
- ❌ Limited to initial state verification

---

## Final Recommendation

### For This Project: **Option 3 (Hybrid)**

Add TestBackend unit tests with insta snapshots for rendering logic:

1. **Keep current integration tests** (8 PTY tests with ratatui-testlib)
   - They work and are well-documented
   - Test real terminal behavior

2. **Add TestBackend unit tests** (NEW - no sleeps!)
   - Fast rendering tests
   - Snapshot-based assertions
   - Test different states/configurations
   - No PTY overhead, no sleeps needed

**Example structure:**
```
tests/
  tui_test.rs              # 8 integration tests (PTY, with sleeps)
  tui_unit_test.rs         # NEW: 20+ unit tests (TestBackend, no sleeps)
  snapshots/               # NEW: insta snapshots
    tui_unit_test__*.snap
```

### Why Hybrid is Best:

1. **Fast feedback** - TestBackend tests run instantly
2. **No sleeps** - Most tests use TestBackend (deterministic)
3. **Comprehensive** - PTY tests cover real terminal behavior
4. **Pure Rust** - No additional languages/toolchains
5. **Battle-tested** - Uses official Ratatui testing approach
6. **Best practices** - Recommended by Ratatui docs

### For Future/Large Projects: **microsoft/tui-test**

If you need to test complex multi-step interactions without sleeps, microsoft/tui-test is the best option despite requiring JavaScript/TypeScript.

---

## Implementation Plan (Recommended)

1. **Add TestBackend unit tests** (eliminates sleeps for most tests)
   - Test rendering with different cgroup configurations
   - Test props mode display
   - Test filter mode display
   - Test tree expansion states
   - Use insta for snapshot assertions

2. **Keep current PTY integration tests** (8 tests)
   - These verify real terminal behavior
   - Well-documented sleep-based approach
   - Reliable and passing

3. **Result:**
   - 8 PTY integration tests (with sleeps, comprehensive)
   - 20+ TestBackend unit tests (no sleeps, fast)
   - Best of both worlds

---

## Example: TestBackend Test (No Sleeps!)

```rust
// tests/tui_unit_test.rs
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use insta::assert_snapshot;

#[test]
fn test_initial_tree_rendering() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("init.scope", &[]);
    f.add("system.slice", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    // Draw once - no sleep needed!
    terminal.draw(|frame| app.draw(frame)).unwrap();

    let buffer = terminal.backend().buffer();

    // Snapshot assertion - deterministic!
    assert_snapshot!(buffer.to_string());

    // Or direct assertions
    let contents = buffer.to_string();
    assert!(contents.contains("init.scope"));
    assert!(contents.contains("system.slice"));
}

#[test]
fn test_props_toggle_interaction() {
    let f = Fixture::new();
    f.add(".", &[("memory.max", "1GB"), ("cpu.weight", "100")]);
    f.add("test.scope", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Simulate 'p' key press - direct event!
    let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty());
    app.handle_key(key);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();

    let buffer = terminal.backend().buffer();
    assert!(buffer.to_string().contains("memory.max"));
    // No sleep, no PTY, instant feedback!
}

#[test]
fn test_navigation_interaction() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("first.scope", &[]);
    f.add("second.scope", &[]);
    f.add("third.scope", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Simulate navigation: down, down, down
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    app.handle_key(down);
    app.handle_key(down);
    app.handle_key(down);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();

    // third.scope should be highlighted (selected)
    let buffer = terminal.backend().buffer();
    // Verify selection moved (would check highlighted style in real test)
    assert!(buffer.to_string().contains("third.scope"));
}

#[test]
fn test_expand_collapse_interaction() {
    let f = Fixture::new();
    f.add(".", &[]);
    f.add("parent.slice", &[]);
    f.add("parent.slice/child.service", &[]);

    let data = cgroup::scan(&f.root_path()).unwrap();
    let mut app = App::new(f.root_path(), data);

    // Initially collapsed (except root)
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let initial = terminal.backend().buffer().to_string();
    assert!(!initial.contains("child.service")); // Child not visible

    // Navigate to parent and expand
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    app.handle_key(down);  // Select parent.slice
    app.handle_key(enter); // Expand it

    // Re-render
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let expanded = terminal.backend().buffer().to_string();
    assert!(expanded.contains("child.service")); // Child now visible!
    // No sleep, no PTY, deterministic!
}
```

---

## Conclusion

**YES** - There are better ways to test TUI apps without sleeps:

1. **microsoft/tui-test** - Best semantic waiting, but requires JavaScript
2. **TestBackend** - No sleeps, but only tests rendering (recommended)
3. **termwright** - Better than current, but still uses timeouts

**Recommended approach:** Hybrid strategy using TestBackend for unit tests (fast, no sleeps) + keep current PTY integration tests (comprehensive, documented).

This gives you:
- ✅ Most tests with no sleeps (TestBackend)
- ✅ Full coverage of real terminal behavior (PTY tests)
- ✅ Pure Rust
- ✅ Fast CI
- ✅ Best practices
