# AI Agent Guidelines

This document provides guidance for AI agents working with the cgtree codebase.

## Testing on macOS

**Important**: `cgtree` requires a Linux system with cgroup v2 to run. If you're working on macOS, you cannot run cgtree directly on the host system.

Before attempting to run or test cgtree, **read [TESTING.md](./TESTING.md)** for complete instructions on how to build and test using Docker.

### Quick Reference

```sh
# Build for Linux in container
docker run --rm --privileged --cgroupns=host \
  -v "$(pwd):/workspace" -w /workspace \
  rust:latest \
  bash -c "cargo build --release"

# Run cgtree in container
docker run --rm --privileged --cgroupns=host \
  -v "$(pwd)/target:/workspace/target:ro" -w /workspace \
  rust:latest \
  /workspace/target/release/cgtree list --props swap,cpu
```

## Key Points

1. **Platform-specific**: cgtree is Linux-only and requires cgroup v2
2. **Docker testing**: On macOS, always use Docker containers for building and testing
3. **Build target**: Building on macOS produces a Darwin binary that won't work in Linux containers - build inside the container instead
4. **Testing guidelines**: See [TESTING.md](./TESTING.md) for comprehensive testing instructions

## Code Structure

- `src/main.rs` - CLI entry point and command parsing
- `src/cgroup.rs` - Core cgroup hierarchy scanning and validation
- `src/data.rs` - Cgroup data structures and file reading
- `src/list.rs` - Tree printing for `list` command
- `src/tui.rs` - Interactive explorer implementation

## Development Workflow

1. Make code changes on the host (macOS/Linux/Windows)
2. Build inside a Linux container (see TESTING.md)
3. Test inside the same container with access to real cgroup v2 hierarchy
4. Run `cargo test` for unit tests (these work on any platform)
5. Run `cargo clippy --all-targets` for linting
