# Integration Test Suite

This directory contains integration tests for `cgtree` that run the actual compiled CLI binary against synthetic cgroup v2 hierarchies.

## Test Architecture

### Components

1. **`Fixture`**: Creates temporary cgroup v2 hierarchies for testing
   - Uses `tempfile` to create isolated temporary directories
   - Each test gets a fresh, isolated hierarchy
   - Automatically cleaned up when the fixture goes out of scope

2. **`run()`**: Executes the cgtree binary with arguments
   - Returns `(stdout, stderr, success)`
   - Simplifies test assertions

### Test Organization

The test suite is organized into categories:

- **List Command Tests**: Basic tree rendering, depth limiting, property filtering
- **JSON Output Tests**: JSON structure validation, property filtering, depth limiting
- **Error Handling Tests**: Invalid paths, wrong cgroup versions, invalid arguments
- **Edge Cases Tests**: Empty hierarchies, deep nesting, many children, concurrent access
- **CLI Behavior Tests**: Help, version, default behavior

## Running Tests

### Run all integration tests

```bash
cargo test --test integration_test
```

### Run only unit tests

```bash
cargo test --bins
```

### Run all tests (unit + integration)

```bash
cargo test
```

### Run a specific test

```bash
cargo test --test integration_test list_basic_tree
```

### Run with output (for debugging)

```bash
cargo test --test integration_test -- --nocapture
```

### Run in verbose mode

```bash
cargo test --test integration_test -- --nocapture --test-threads=1
```

## Test Setup

### Prerequisites

- Rust toolchain (edition 2024)
- Dependencies:
  - `tempfile` (for temporary directories)
  - `serde_json` (for JSON validation)

These are listed in `Cargo.toml` under `[dev-dependencies]`.

### How Tests Work

1. **Fixture Creation**: Each test creates a `Fixture` which sets up a temporary directory
2. **Cgroup Creation**: Tests call `fixture.add(path, fields)` to create fake cgroups
   - Each cgroup gets `cgroup.controllers` and `cgroup.procs` files automatically
   - Optional custom fields can be added (e.g., `memory.max`, `cpu.weight`)
3. **CLI Execution**: Tests call `run(&[args...])` to execute the binary
4. **Assertion**: Tests verify stdout, stderr, and exit status

### Example Test

```rust
#[test]
fn list_basic_tree() {
    // 1. Create fixture
    let f = Fixture::new();

    // 2. Build hierarchy
    f.add(".", &[]);                           // Root cgroup
    f.add("system.slice", &[]);                // Child cgroup
    f.add("system.slice/ssh.service", &[]);    // Grandchild

    // 3. Run CLI
    let (out, _, ok) = run(&["--root", f.root().to_str().unwrap(), "list"]);

    // 4. Assert results
    assert!(ok);
    assert!(out.contains("system.slice"));
    assert!(out.contains("ssh.service"));
}
```

## Writing New Tests

### Test Naming Convention

Use descriptive names that indicate what is being tested:
- `list_*`: Tests for the list command
- `json_*`: Tests for JSON output
- `error_*`: Tests for error cases

### Best Practices

1. **Keep tests focused**: Each test should verify one specific behavior
2. **Use fresh fixtures**: Always create a new `Fixture` per test
3. **Assert thoroughly**: Check both what should and shouldn't appear in output
4. **Test error cases**: Verify that invalid inputs fail correctly
5. **Add descriptive messages**: Use assertion messages to aid debugging

### Adding Custom Fields

To test property filtering, add custom fields:

```rust
f.add(".", &[
    ("memory.max", "max"),
    ("cpu.weight", "100"),
    ("custom.field", "value with spaces"),
]);
```

### Testing Multiline Values

Some cgroup fields have multiline output:

```rust
f.add(".", &[("memory.stat", "anon 1024\nfile 2048\nshmem 512")]);
```

## Debugging Failed Tests

### 1. Run with output

```bash
cargo test --test integration_test test_name -- --nocapture
```

### 2. Check actual vs expected output

Add debug prints to your test:

```rust
let (out, err, ok) = run(&[...]);
eprintln!("stdout: {}", out);
eprintln!("stderr: {}", err);
eprintln!("success: {}", ok);
```

### 3. Inspect temporary directories

Modify the test to keep the temp directory:

```rust
let f = Fixture::new();
eprintln!("Test root: {:?}", f.root());
std::thread::sleep(std::time::Duration::from_secs(60)); // Inspect filesystem
```

### 4. Run with backtrace

```bash
RUST_BACKTRACE=1 cargo test --test integration_test test_name
```

## Test Coverage

Current test count: **20 tests**

Coverage breakdown:
- List command: 7 tests
- JSON output: 3 tests
- Error handling: 3 tests
- Edge cases: 5 tests
- CLI behavior: 3 tests

All tests run in ~0.2 seconds and are fully isolated from each other.

## CI/CD Integration

These tests are suitable for CI/CD pipelines:

- **Fast**: Complete suite runs in under 1 second
- **Isolated**: No shared state between tests
- **Deterministic**: No flaky tests or race conditions
- **Self-contained**: No external dependencies required

### Example GitHub Actions

```yaml
- name: Run tests
  run: cargo test --all
```

### Example GitLab CI

```yaml
test:
  script:
    - cargo test --all
```

## Maintenance

### When to Update Tests

- **New CLI feature**: Add tests for the new functionality
- **Bug fix**: Add regression tests
- **Breaking change**: Update existing tests to match new behavior
- **Performance improvement**: Consider adding benchmark tests

### Keeping Tests Simple

- Prefer simple, focused tests over complex multi-assertion tests
- Avoid test interdependencies
- Use helper functions to reduce duplication
- Document any non-obvious test logic

## Related Documentation

- Main README: `../README.md`
- Development guide: See "Development" section in main README
- Unit tests: Inline in `src/*.rs` files (run with `cargo test`)
