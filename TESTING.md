# Testing cgtree with Linux VMs and Containers

This guide covers the most minimal ways to run `cgtree` in a Linux environment with cgroup v2 support, particularly useful when developing on macOS or other non-Linux systems.

## Overview

`cgtree` requires a Linux system with cgroup v2 (unified hierarchy) to function properly. The tool inspects `/sys/fs/cgroup` by default, which is only available on Linux systems with cgroup v2 enabled.

## Quick Start: Recommended Approaches

### Option 1: Docker Desktop (Simplest for Most Users)

Docker Desktop on macOS has used cgroup v2 by default since version 4.3.0.

**Setup:**
```sh
# Install Docker Desktop from https://www.docker.com/products/docker-desktop

# Verify cgroup v2 is enabled
docker info --format '{{.CgroupVersion}}'  # Should output: 2
```

**Running cgtree:**
```sh
# Build the binary (on macOS)
cargo build --release

# Run in Alpine Linux container with privileged access to cgroups
docker run --rm -it \
  --privileged \
  -v "$(pwd)/target/release/cgtree:/usr/local/bin/cgtree:ro" \
  alpine:latest \
  /bin/sh

# Inside the container, install dependencies and run
apk add --no-cache bash
cgtree list
cgtree  # interactive mode
```

**For a more realistic cgroup hierarchy:**
```sh
# Run a container with systemd to get a proper cgroup tree
docker run --rm -it \
  --privileged \
  --cgroupns=host \
  -v "$(pwd)/target/release/cgtree:/usr/local/bin/cgtree:ro" \
  ubuntu:24.04 \
  /bin/bash

# Inside the container
cgtree list --procs
```

### Option 2: Colima (Lightweight Alternative to Docker Desktop)

Colima provides container runtimes on macOS with minimal setup and cgroup v2 support (from v0.6.0+).

**Setup:**
```sh
# Install Colima
brew install colima

# Start Colima with Docker runtime
colima start

# Verify cgroup v2
docker info --format '{{.CgroupVersion}}'
```

**Running cgtree:**
```sh
# Same as Docker Desktop approach above
docker run --rm -it \
  --privileged \
  -v "$(pwd)/target/release/cgtree:/usr/local/bin/cgtree:ro" \
  alpine:latest \
  cgtree list
```

### Option 3: Lima (Minimal Linux VMs)

Lima launches Linux VMs on macOS with automatic file sharing and supports cgroup v2.

**Setup:**
```sh
# Install Lima
brew install lima

# Start a default VM (Ubuntu-based)
limactl start

# Or create a custom minimal Alpine VM
cat > alpine-minimal.yaml <<EOF
images:
  - location: "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/aarch64/alpine-virt-3.20.3-aarch64.iso"
    arch: "aarch64"
cpus: 2
memory: "2GiB"
mounts:
  - location: "~"
    writable: false
  - location: "~/dev/cgtree"
    writable: true
EOF

limactl start alpine-minimal.yaml
```

**Running cgtree:**
```sh
# SSH into the VM
lima

# Navigate to your project (auto-mounted)
cd ~/dev/cgtree

# Build and run
cargo build --release
./target/release/cgtree list
```

### Option 4: Multipass (Ubuntu VMs)

Multipass provides fast Ubuntu VMs with minimal configuration.

**Setup:**
```sh
# Install Multipass
brew install --cask multipass

# Launch an Ubuntu 24.04 VM (has cgroup v2 by default)
multipass launch 24.04 --name cgtree-test --memory 2G --disk 10G

# Mount your project directory
multipass mount $(pwd) cgtree-test:/home/ubuntu/cgtree
```

**Running cgtree:**
```sh
# Shell into the VM
multipass shell cgtree-test

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Build and run
cd ~/cgtree
cargo build --release
./target/release/cgtree list --procs
```

## Comparison Matrix

| Method | Setup Time | Resource Usage | cgroup v2 Default | Best For |
|--------|-----------|----------------|-------------------|----------|
| Docker Desktop | 5 min | Medium | Yes (v4.3.0+) | Quick testing, already using Docker |
| Colima | 2 min | Low | Yes (v0.6.0+) | Lightweight Docker alternative |
| Lima | 3 min | Low | Yes | Custom Linux environments |
| Multipass | 5 min | Medium | Yes (Ubuntu 24.04+) | Full Ubuntu environment |

## Creating a Realistic Test Hierarchy

For testing with a more complex cgroup tree:

```sh
# Inside any of the above Linux environments
docker run -d --name test-app nginx
docker run -d --name test-db postgres:alpine

# Now run cgtree to see the hierarchy
cgtree list --procs
```

## Verifying cgroup v2

Inside any Linux environment, verify cgroup v2 is properly configured:

```sh
# Check if cgroup v2 is mounted
mount | grep cgroup2
# Should show: cgroup2 on /sys/fs/cgroup type cgroup2 (rw,...)

# Check for the unified hierarchy marker
ls /sys/fs/cgroup/cgroup.controllers
# Should exist and show available controllers

# View available controllers
cat /sys/fs/cgroup/cgroup.controllers
# Output example: cpuset cpu io memory hugetlb pids rdma misc
```

## Troubleshooting

### "No such file or directory" for /sys/fs/cgroup
- Ensure you're running on a Linux system (not macOS/Windows host)
- Verify cgroup v2 is enabled (see verification steps above)

### Permission Denied
```sh
# Run with sudo or use --privileged flag for containers
sudo ./cgtree list

# Or in Docker:
docker run --privileged ...
```

### Empty cgroup hierarchy
```sh
# Create some test cgroups
sudo mkdir -p /sys/fs/cgroup/test.slice
echo $$ | sudo tee /sys/fs/cgroup/test.slice/cgroup.procs

# Now run cgtree
cgtree list
```

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
