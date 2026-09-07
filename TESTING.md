# Testing cgtree on macOS

`cgtree` requires a Linux system with cgroup v2. On macOS, use Docker to build and run it in a container.

## Quick Start

**Important**: Building with `cargo build` on macOS creates a macOS binary that won't work in Linux containers. You must build inside a Linux container.

```sh
# Build and test in a single Linux container
docker run --rm -it \
  --cgroupns=host \
  -v "$(pwd):/workspace" \
  -w /workspace \
  rust:latest \
  bash

# Inside the container:
cargo build --release
./target/release/cgtree list
./target/release/cgtree list -p swap,cpu
./target/release/cgtree list -p "*"  # Show all fields
./target/release/cgtree explore  # Interactive TUI with field filtering
exit
```

### Interactive TUI Mode

Use `↑`/`↓` or `j`/`k` to navigate, `Enter`/`Space` to expand/collapse, `q` to quit.

Press `f` to activate field filtering, then type comma-separated patterns (e.g., `memory,cpu`) and press Enter to display matching fields under each node.

Docker Desktop on macOS uses cgroup v2 by default (v4.3.0+).

## References

- [Docker Desktop cgroup v2 support](https://docs.docker.com/desktop/containerd/#enabling-cgroup-v2)
- [Linux cgroup v2 Documentation](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
