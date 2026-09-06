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
./target/release/cgtree view  # Interactive TUI
exit
```

### Interactive TUI Mode

Use `↑`/`↓` or `j`/`k` to navigate, `Enter` to expand/collapse, `Tab` to switch panes, `q` to quit.

Docker Desktop on macOS uses cgroup v2 by default (v4.3.0+).

## References

- [Docker Desktop cgroup v2 support](https://docs.docker.com/desktop/containerd/#enabling-cgroup-v2)
- [Linux cgroup v2 Documentation](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
