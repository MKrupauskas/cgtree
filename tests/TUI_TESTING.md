# TUI Integration Testing Guide

## Overview

This project uses `ratatui-testlib` for PTY-based TUI testing. The tests spawn the actual TUI in a pseudo-terminal, send keyboard input, and verify the rendered output.

## Quick Start

```rust
use ratatui_testlib::{TuiTestHarness, KeyCode, CommandBuilder};

#[test]
fn test_tui() -> ratatui_testlib::Result<()> {
    // 1. Create PTY harness
    let mut harness = TuiTestHarness::new(80, 24)?;

    // 2. Spawn your TUI
    let mut cmd = CommandBuilder::new("./my-tui-app");
    harness.spawn(cmd)?;

    // 3. Wait for initial render
    std::thread::sleep(Duration::from_millis(200));

    // 4. Send 'q' to quit
    harness.send_key(KeyCode::Char('q'))?;
    std::thread::sleep(Duration::from_millis(50));

    // 5. Verify output
    let contents = harness.screen_contents();
    assert!(contents.contains("Expected text"));

    Ok(())
}
```

## The Working Pattern

### ✅ DO This

```rust
// Spawn, sleep, quit, verify
let mut harness = spawn_tui(&root)?;
harness.send_key(KeyCode::Char('q'))?;
std::thread::sleep(Duration::from_millis(50));
let contents = harness.screen_contents();
assert!(contents.contains("test"));
```

### ❌ DON'T Do This

```rust
// Multiple send_key calls - can hang on macOS
harness.send_key(KeyCode::Char('j'))?;
harness.send_key(KeyCode::Char('j'))?;
harness.send_key(KeyCode::Char('q'))?;

// wait_for before sending keys - will block
harness.wait_for(|state| state.contents().contains("text"))?;
harness.send_key(KeyCode::Char('q'))?;

// update_state before sending keys - will block
harness.update_state()?;
harness.send_key(KeyCode::Char('q'))?;
```

## Why This Pattern?

### The TUI Event Loop

```rust
loop {
    terminal.draw(|frame| self.draw(frame))?;  // Draws once
    if let Event::Key(key) = event::read()?    // Then BLOCKS here
        && self.handle_key(key) {
        return Ok(());
    }
}
```

After the initial draw, the TUI blocks waiting for keyboard input. The test pattern works around this:

1. **Sleep 200ms** - Gives TUI time to complete first draw cycle
2. **Send 'q' once** - Unblocks `event::read()`, allows clean exit
3. **Sleep 50ms** - Gives time for graceful shutdown
4. **Read contents** - PTY buffer contains the rendered output

### Why Multiple `send_key()` Calls Fail

Each `send_key()` internally calls:
1. Encode key to bytes
2. Write to PTY
3. Sleep 50ms
4. **Call `update_state()`** ← This can block on macOS

The `update_state()` call does:
```rust
loop {
    match self.terminal.read(&mut buf) {
        Ok(0) => break,           // No data - exit
        Ok(n) => process data,
        Err(WouldBlock) => break,
    }
}
```

On macOS, `try_clone_reader()` (called inside `read()`) can block when called repeatedly, causing tests to hang.

## Current Test Suite

### 5 Passing Tests

1. **tui_starts_and_shows_tree** - Verifies tree rendering with multiple cgroups
2. **tui_shows_help_footer** - Checks footer contains help hints
3. **tui_shows_directory_indicator** - Verifies directories have "/" suffix
4. **tui_shows_multiple_children** - Tests displaying 3+ child cgroups
5. **tui_renders_with_properties** - Tests with cgroup property data

### Test Coverage

**What's tested:**
- ✅ Initial rendering
- ✅ Tree structure display
- ✅ Footer text
- ✅ Different data configurations
- ✅ Property hints

**What's not tested:**
- ⚠️ Navigation (j/k/arrows)
- ⚠️ Expand/collapse (space/enter)
- ⚠️ Props toggling ('p' key)
- ⚠️ Filter mode ('f' key)
- ⚠️ Help screen ('?' key)

**Why?** These require multiple key presses, which trigger the macOS PTY blocking issue.

## Alternative: Testing Interactive Flows

If you need to test complex interactions:

### Option 1: Expect Scripts

```tcl
#!/usr/bin/expect
spawn ./cgtree explore
expect "init.scope"
send "j"
expect "system.slice"
send "q"
expect eof
```

### Option 2: TestBackend (Unit Tests)

```rust
use ratatui::backend::TestBackend;

let backend = TestBackend::new(80, 24);
let mut terminal = Terminal::new(backend)?;
terminal.draw(|f| app.draw(f))?;

let buffer = terminal.backend().buffer();
assert_eq!(buffer.get(0, 0).symbol(), "t");
```

### Option 3: Manual Testing

Sometimes the best test is a human! Interactive TUIs benefit from exploratory testing.

## Troubleshooting

### Test Hangs

**Symptom:** Test runs but never completes

**Causes:**
- Multiple `send_key()` calls
- Calling `wait_for()` / `update_state()` before sending keys
- Not sending 'q' to quit

**Fix:** Use the simple pattern (spawn → sleep → quit → verify)

### No Output Captured

**Symptom:** `screen_contents()` returns empty string

**Causes:**
- Not sleeping before quitting (TUI hasn't drawn yet)
- TUI crashed/exited immediately

**Fix:**
- Check TUI didn't error (look for stderr output)
- Increase initial sleep to 300ms
- Verify TUI works manually

### Flaky Tests

**Symptom:** Tests pass sometimes, fail others

**Causes:**
- Sleep timings too tight
- Running tests in parallel

**Fix:**
- Increase sleep durations (200ms → 300ms)
- Run with `--test-threads=1`

## Best Practices

1. **Keep it simple** - Test initial rendering, not complex flows
2. **One key press** - Quit ('q') to exit cleanly
3. **Use sleeps** - They're simple and reliable for <300ms waits
4. **Complement with CLI tests** - Test logic separately
5. **Document trade-offs** - Be clear about what you're not testing

## Resources

- [ratatui-testlib](https://crates.io/crates/ratatui-testlib)
- [portable-pty](https://docs.rs/portable-pty)
- [Testing TUI Apps Blog](https://blog.waleedkhan.name/testing-tui-apps/)
- Project tests: `tests/tui_test.rs`
