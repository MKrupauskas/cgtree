# Testing cgtree on macOS

`cgtree` requires a Linux system with cgroup v2. On macOS, use Docker to run it in a container.

## Quick Start

```sh
# Build the binary
cargo build --release

# Run in Ubuntu container (has realistic cgroup hierarchy)
docker run --rm -it \
  --privileged \
  --cgroupns=host \
  -v "$(pwd)/target/release/cgtree:/usr/local/bin/cgtree:ro" \
  ubuntu:24.04 \
  cgtree list --procs
```

That's it. Docker Desktop on macOS uses cgroup v2 by default (v4.3.0+).

## CI/CD Testing

For automated testing in CI environments:

```yaml
# GitHub Actions example
name: Test cgtree
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-24.04  # Has cgroup v2 by default
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build
        run: cargo build --release
      - name: Test list command
        run: ./target/release/cgtree list
      - name: Test with depth limit
        run: ./target/release/cgtree list --depth 2
```

## References

- [Docker Desktop cgroup v2 support](https://docs.docker.com/desktop/containerd/#enabling-cgroup-v2)
- [Colima Documentation](https://github.com/abiosoft/colima)
- [Lima Documentation](https://lima-vm.io/)
- [Multipass Documentation](https://multipass.run/)
- [Linux cgroup v2 Documentation](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
