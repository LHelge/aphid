---
title: Development Setup
category: Development
tags:
  - guide
---

This page walks through everything needed to build and run aphid from source on your local machine.

# Prerequisites

## Rust toolchain

aphid is written in Rust. Install the toolchain via [rustup](https://rustup.rs/):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the on-screen prompts to complete the installation. Once finished, make sure `~/.cargo/bin` is on your `PATH` (rustup normally sets this up for you). Verify the installation:

```sh
rustc --version
cargo --version
```

aphid requires **Rust 1.75 or later**. If you already have Rust installed, make sure you are up to date:

```sh
rustup update
```

## Git

You will need [Git](https://git-scm.com/) to clone the repository. Most systems ship with it, but you can verify:

```sh
git --version
```

On macOS you can install it via Xcode Command Line Tools (`xcode-select --install`) or [Homebrew](https://brew.sh/) (`brew install git`). On Linux, use your distribution's package manager.

# Clone and build

```sh
git clone https://github.com/LHelge/aphid
cd aphid
cargo build
```

The first build downloads and compiles all dependencies, so it takes a while. Subsequent builds are incremental and much faster.

# Run the test suite

```sh
cargo test
```

To run a single test by name:

```sh
cargo test <test_name_substring>
```

# Linting and formatting

Before committing, always run:

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
```

These are the same checks that CI enforces on every pull request.

# Running locally

The repository includes a documentation site under `docs/` that doubles as a convenient test fixture. Build it:

```sh
cargo run -- --config docs/aphid.toml build
```

Or start the dev server with live reload:

```sh
cargo run -- --config docs/aphid.toml serve
```

Then open [http://localhost:3000](http://localhost:3000) in your browser. Any changes to content or templates trigger an automatic rebuild and browser refresh.

# Next steps

- Read [[contributing]] for the pull request workflow and commit conventions.
- See [[cli]] for the full list of commands and flags.
- Browse the [[configuration]] reference to understand `aphid.toml`.
