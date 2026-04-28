---
title: Installation
category: Getting Started
tags:
  - reference
---

# Requirements

- Rust 1.75 or later (for building from source)
- Any modern OS: Linux, macOS, Windows

# Install with Cargo

The recommended way to install aphid is via Cargo:

```sh
cargo install aphid --locked
```

This compiles and installs the latest release from [crates.io](https://crates.io/crates/aphid). The `aphid` binary is placed in `~/.cargo/bin/`, which should already be on your `PATH` if you installed Rust via `rustup`.

# Build from source

See [[development-setup]] for prerequisites and detailed instructions on getting the toolchain ready. Once you have Rust installed:

```sh
git clone https://github.com/LHelge/aphid
cd aphid
cargo install --path .
```

# Verify

```sh
aphid --version
```

# Usage

```sh
aphid                          # start a dev server on :3000 (default command)
aphid serve                    # explicit form, same as `aphid`
aphid serve --port 8080        # custom port
aphid build                    # render the site into dist/ (use this in CI)
aphid --config path/to/aphid.toml build   # use a custom config
```

See [[cli]] for the full command reference and [[configuration]] for the `aphid.toml` reference.
