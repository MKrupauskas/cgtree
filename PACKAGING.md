# Packaging

`cgtree` is distributed as a Debian package built by
[`cargo-deb`](https://github.com/kornelski/cargo-deb) from the
`[package.metadata.deb]` section of `Cargo.toml`. There is no `debian/rules`
or `dpkg-buildpackage` involved; the only hand-written Debian file is
`debian/changelog`.

## Releasing

Releases are cut by pushing a tag:

```sh
# 1. Bump the version in Cargo.toml and add a debian/changelog entry
#    (the workflow fails the build if the two disagree with the tag).
# 2. Land that on main, then:
git tag v0.2.0
git push origin v0.2.0
```

`.github/workflows/release.yml` then builds a `.deb` for amd64 and arm64,
installs and smoke-tests the amd64 one on the runner's live cgroup v2
hierarchy, and publishes both packages plus SHA256 checksums to a GitHub
release.

`workflow_dispatch` runs everything except the publish step, which is the way
to test packaging changes without cutting a release.

## Building locally

`cargo-deb` needs Linux, so on macOS build in a container:

```sh
docker run --rm -v "$(pwd):/w" -w /w rust:latest bash -c '
  apt-get update -qq && apt-get install -y -qq musl-tools
  rustup target add x86_64-unknown-linux-musl
  cargo install cargo-deb --locked
  cargo deb --target x86_64-unknown-linux-musl --profile deb'
```

Inspect and check the result:

```sh
dpkg-deb -c target/debian/cgtree_*.deb   # contents
lintian target/debian/cgtree_*.deb       # policy checks
```

Three lintian warnings are expected and intentional:

- `empty-field Depends` and `shared-library-lacks-prerequisites` — the binary
  is statically linked against musl, so it genuinely has no dependencies.
- `initial-upload-closes-no-bugs` — only meaningful for uploads into Debian
  proper, which this is not.

Note that `debian:*-slim` container images set `path-exclude /usr/share/man/*`
and `/usr/share/doc/*` in `/etc/dpkg/dpkg.cfg.d/docker`. Man pages will appear
missing after `dpkg -i` in such an image unless that file is removed first;
this is the image's doing, not the package's.

## Why musl

The release build targets `*-unknown-linux-musl` and links statically.
Building against glibc on a current runner bakes in symbol versions that
older Debian and Ubuntu releases cannot resolve, so a static binary is what
makes one `.deb` work across releases. The `[profile.deb]` profile adds LTO
and stripping to keep that binary small.

## Man pages and completions

`build.rs` generates `cgtree.1`, the per-subcommand man pages, and the bash,
zsh and fish completion scripts into `target/dist/` from the same
`clap::Command` the binary parses with. This is why the CLI definition lives
in `src/cli.rs` (library) rather than in `src/main.rs` — a build script cannot
link against its own crate's library, so it includes that module directly.

Nothing needs regenerating by hand; editing `src/cli.rs` is enough.

## Adding other distributions

The layout is deliberately additive. The binary, man pages and completions are
already produced as plain files in `target/dist/`, so another format mostly
means another metadata block and another matrix entry:

- **RPM** (Fedora, RHEL, openSUSE) —
  [`cargo-generate-rpm`](https://github.com/cat-in-136/cargo-generate-rpm)
  reads `[package.metadata.generate-rpm]` and needs no `rpmbuild`. Note that
  `cargo-rpm` is end-of-life; this is its replacement.
- **Arch** — a `PKGBUILD` in the AUR, typically a `-bin` package that unpacks
  the released musl binary.
- **Alpine** — an `APKBUILD`, or `.apk` via nFPM.

If the count of formats grows past two or three,
[`cargo-nfpm`](https://github.com/RagnarLab/cargo-nfpm) covers deb, rpm, apk,
archlinux and ipk from a single `[package.metadata.nfpm]` block. That trades
per-format fidelity for one config, so it is worth switching to only once
maintaining several separate blocks becomes the larger cost.
