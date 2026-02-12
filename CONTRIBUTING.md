# Contributing to ghdash

Thank you for your interest in contributing to ghdash! This document provides guidelines to help you get started.

## Getting Started

1. Fork the repository and clone your fork:

   ```sh
   git clone https://github.com/your-username/ghdash.git
   cd ghdash
   ```

2. Make sure you have Rust installed (1.85+ required for edition 2024):

   ```sh
   rustup update stable
   ```

3. Build and run the tests:

   ```sh
   cargo build
   cargo test
   ```

## Development Workflow

### Running locally

```sh
# Create a config file
mkdir -p ~/.config/ghdash
cat > ~/.config/ghdash/config.toml << 'EOF'
[github]
users = ["your-github-username"]
EOF

# Run in debug mode
cargo run -- --debug
```

### Code quality checks

Before submitting a PR, make sure all checks pass:

```sh
cargo fmt --all -- --check   # Formatting
cargo clippy --all-targets   # Linting
cargo test --all-targets     # Tests
cargo build --release        # Release build
```

CI runs these same checks on every pull request.

### Project structure

```
src/
  app/         App state machine, actions, reducer, event loop, layout
  github/      GraphQL client, queries, models, auth
  cache/       Disk cache with TTL
  ui/          Theme constants, widget rendering functions
  util/        Config loader, time formatting, browser helper
tests/         Integration tests
```

### Key design decisions

- **Action Channel pattern** — All events (keyboard, API responses) flow through a single `mpsc` channel as `Action`s. The `update()` function is a pure state reducer that returns `SideEffect`s, keeping state transitions testable.
- **Bounded concurrency** — A `tokio::sync::Semaphore(4)` limits concurrent GitHub API requests.
- **Client-side search** — Search filters the already-fetched PR list by substring matching. No additional API calls.
- **Flat nav tree** — The navigation tree is a `Vec<NavNode>` rebuilt from org data whenever it changes, rather than a recursive tree structure.

## Making Changes

1. Create a branch for your change:

   ```sh
   git checkout -b my-feature
   ```

2. Make your changes. Follow the existing code style — `cargo fmt` and `cargo clippy` enforce this.

3. Add tests for new functionality. Tests live in `tests/` as integration tests. The main test suites are:
   - `config_tests.rs` — Config parsing and defaults
   - `cache_tests.rs` — Cache set/get/TTL/invalidation
   - `state_tests.rs` — State transitions and the update reducer
   - `time_tests.rs` — Relative time formatting
   - `graphql_parse_tests.rs` — Model serialization and accessors

4. Run the full check suite:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets
   cargo test --all-targets
   ```

5. Commit and push your branch, then open a pull request.

## Pull Request Guidelines

- Keep PRs focused — one feature or fix per PR.
- Write a clear description of what changed and why.
- Ensure CI passes before requesting review.
- Add tests for bug fixes and new features.

## Reporting Issues

- Use [GitHub Issues](https://github.com/zombocoder/ghdash/issues) to report bugs or request features.
- Include your OS, Rust version (`rustc --version`), and steps to reproduce.

## License

By contributing to ghdash, you agree that your contributions will be licensed under the Apache License 2.0.
