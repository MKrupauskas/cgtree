# Linux Container Testing - Research Summary

## Question
Would TUI tests be more robust if run in a Linux container instead of macOS?

## Answer: YES - Linux containers would be more reliable

### Why macOS Has PTY Issues

Research and testing revealed that **macOS has platform-specific PTY blocking issues**:

1. **`try_clone_reader()` blocks on macOS**
   - When calling multiple times while child process is in blocking read
   - Linux handles this more gracefully

2. **Platform-specific bugs**
   - Issue: "ptmx.Read() blocks on Darwin indefinitely" (GitHub creack/pty#114)
   - `posix_openpt()` has limitations on macOS vs Linux
   - Setting O_NONBLOCK can fail with "Inappropriate ioctl for device"

3. **Different read behavior**
   - macOS returns 0 on closed PTY, Linux returns -1 with I/O error
   - Fundamental differences in PTY subsystem implementation

### Current Test Approach

Our tests work around macOS limitations by:
- Using timeout-based synchronization (200ms for startup)
- Limiting to ONE `send_key()` call per test (to avoid repeated PTY reads)
- Tests are **reliable but conservative**

### Benefits of Linux Containers

Running tests in Docker/Podman on Linux would enable:

✅ **More robust PTY handling**
- `try_clone_reader()` works reliably
- Multiple `send_key()` calls would work
- Could use `wait_for()` polling without blocking

✅ **Test complex interactions**
- Multi-step keyboard sequences (navigate, expand, filter, quit)
- Currently impossible on macOS without hanging

✅ **CI/CD consistency**
- Same environment locally and in CI
- No platform-specific workarounds

### Implementation Example

```bash
# Add to project
docker run --rm -v $(pwd):/workspace -w /workspace rust:latest cargo test

# Or use .github/workflows
- uses: docker://rust:latest
  run: cargo test --test tui_test
```

### Trade-offs

**Pros:**
- More reliable PTY behavior
- Can test complex interactions
- Better CI consistency

**Cons:**
- Requires Docker/Podman
- Slightly slower (container overhead)
- One more dependency to manage

### Recommendation

**For this project:** Current approach is fine
- 8 TUI tests pass reliably on macOS
- Tests verify rendering and basic functionality
- Timeout-based sync is well-documented and standard practice

**For future/larger projects:** Consider Linux containers if:
- Need to test complex multi-step interactions
- Running in CI anyway (no local burden)
- Want maximum reliability for PTY testing

## Current Test Results

```
59 total tests passing:
- 26 unit tests
- 25 CLI integration tests
- 8 TUI integration tests (reliable on macOS with timeout-based sync)
```

## References

- GitHub Issue: "ptmx.Read() blocks on Darwin" - creack/pty#114
- Apple Forums: "Setting non-blocking on a pty not working"
- Medium: "How macOS PTY Works" - Detailed macOS PTY behavior
- LKML: "Inconsistency between PTY read() return values" - Platform differences
