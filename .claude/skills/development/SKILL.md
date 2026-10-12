---
name: development

description: >
  This skill defines general software-development practices for Rust projects.
  Architecture and architectural patterns are defined by the separate `architecture` skill.
  Documentation practices are defined by the separate `documentation` skill.
  Do not introduce architectural decisions through this skill unless explicitly required by the project or another applicable skill.
---

# 1. Rust Version

Use **Rust 1.99.0** (stable) and the **2024 edition** for this project.

Verify the installed version when necessary:

```bash
rustc --version
cargo --version
```

Pin the toolchain in `rust-toolchain.toml` at the repository root:

```toml
[toolchain]
channel = "1.99.0"
components = ["rustfmt", "clippy", "rust-analyzer"]
```

Declare the edition and minimum supported Rust version in `Cargo.toml` (in `[workspace.package]` for workspaces):

```toml
[package]
edition = "2024"
rust-version = "1.99"
```

Use modern Rust features when they improve the code.

Do not use advanced type-system features (complex trait bounds, GATs, const generics, heavy macro use) merely because they are available.

Prefer straightforward concrete Rust code when it is clearer.

---

# 2. Idiomatic Rust

Write idiomatic Rust.

Prefer:

* simple code
* explicit control flow
* small functions
* meaningful names
* small, focused traits
* explicit error handling with `Result`
* ownership and borrowing over shared mutable state
* composition over unnecessary abstraction
* standard-library solutions where practical
* making invalid states unrepresentable through types

Avoid:

* unnecessary abstractions
* excessive traits
* deeply nested control flow
* clever one-liners and long iterator chains that obscure intent
* premature optimization
* unnecessary `clone()` to satisfy the borrow checker
* unnecessary `Rc<RefCell<_>>` / `Arc<Mutex<_>>`
* unnecessary generics
* unnecessary macros
* `unsafe` code
* global mutable state

Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and standard naming conventions unless the project has a documented reason to do otherwise.

---

# 3. TDD

Use **Test-Driven Development** for new functionality.

Follow:

```text
RED
  ↓
Write a failing test
  ↓
GREEN
  ↓
Implement the minimum required behavior
  ↓
REFACTOR
  ↓
Improve the implementation while keeping tests green
```

For new behavior:

1. Define the expected behavior.
2. Write the test first.
3. Run the test and verify that it fails for the expected reason.
4. Implement the minimum required code.
5. Run the test.
6. Refactor if necessary.
7. Run the broader test suite.

Do not routinely implement a large feature first and add tests afterward.

Tests should influence API design and make code easier to use and maintain.

---

# 4. Test Tooling

Use Rust's built-in test harness (`#[test]`, `cargo test`) as the foundation.

Use the standard assertion macros:

```rust
assert_eq!(actual, expected);
assert_ne!(actual, other);
assert!(result.is_err());
assert!(matches!(err, ParseError::Empty));
```

Use these crates as dev-dependencies when they add value:

```text
rstest              parameterized (table-driven) tests and fixtures
pretty_assertions   readable diffs for assert_eq! on large values
mockall             mocking trait-based dependencies
```

Tests may return `Result` so that `?` can be used instead of `unwrap()`:

```rust
#[test]
fn loads_configuration() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_str("name = \"example\"")?;
    assert_eq!(config.name, "example");
    Ok(())
}
```

Use `expect("reason")` rather than bare `unwrap()` in tests when a failure message helps.

Use mocking when it is appropriate.

Do not mock everything.

Prefer real implementations, simple fakes, or in-memory implementations when they make tests clearer and more reliable.

---

# 5. Test Organization

Keep unit tests close to the code they test, in a `tests` module in the same file:

```rust
pub fn parse(input: &str) -> Result<Value, ParseError> {
    // ...
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_value() {
        // test
    }
}
```

Unit tests may exercise private items when testing internal behavior is appropriate.

Put tests of the public API in the crate's `tests/` directory (integration tests), which compile as separate crates and can only use the public API:

```text
crate/
├── Cargo.toml
├── src/
│   └── lib.rs
└── tests/
    ├── common/
    │   └── mod.rs
    └── parse.rs
```

Use doc tests on public items when examples double as documentation.

---

# 6. Table-Driven Tests

Prefer parameterized tests when multiple inputs exercise the same behavior.

Example using `rstest`:

```rust
use rstest::rstest;

#[rstest]
#[case::valid_value("example", Ok(Value::from("example")))]
#[case::empty_value("", Err(ParseError::Empty))]
fn parse_cases(#[case] input: &str, #[case] expected: Result<Value, ParseError>) {
    assert_eq!(parse(input), expected);
}
```

Without extra dependencies, iterate over a slice of cases and include the case name in the assertion message:

```rust
#[test]
fn parse_cases() {
    let cases = [
        ("valid value", "example", Ok(Value::from("example"))),
        ("empty value", "", Err(ParseError::Empty)),
    ];

    for (name, input, expected) in cases {
        assert_eq!(parse(input), expected, "case: {name}");
    }
}
```

Use individual tests when they communicate the behavior more clearly.

Do not force unrelated cases into one table.

---

# 7. Test Naming

Test names should describe behavior.

Prefer:

```rust
#[test]
fn parse_returns_error_for_empty_input()
```

over:

```rust
#[test]
fn test_parse2()
```

Use descriptive `#[case::name]` labels for parameterized cases.

Tests should make failures understandable without reading the implementation.

---

# 8. Test Quality

Tests should verify behavior, not implementation details unnecessarily.

Prefer:

```text
given input
    ↓
expected behavior
```

over tests that merely reproduce the implementation's internal algorithm.

Good tests should be:

* deterministic
* isolated
* readable
* fast where possible
* repeatable
* independent of execution order

`cargo test` runs tests in parallel; tests must not share mutable state such as files, environment variables, or ports without isolation.

Avoid tests that depend on:

* arbitrary sleeps
* timing assumptions
* global mutable state
* process-wide environment variables
* uncontrolled randomness
* external services unless explicitly an integration test

---

# 9. Testing Levels

Use the appropriate type of test for the behavior.

### Unit tests

Use for isolated logic and components.

### Integration tests

Use when verifying interaction with real dependencies such as:

* databases
* message brokers
* filesystem
* external services
* network components

Mark slow or environment-dependent tests with `#[ignore]` and run them explicitly via a Makefile target.

### End-to-end tests

Use for important complete workflows.

Do not push all testing into E2E tests.

Most behavior should be covered by fast unit tests, with integration and E2E tests covering boundaries and critical workflows.

---

# 10. Memory and Concurrency Safety Checks

Safe Rust prevents data races at compile time. The remaining risks are `unsafe` code, logic races, and deadlocks.

For crates containing `unsafe` code, run the tests under Miri:

```bash
rustup toolchain install nightly --component miri
cargo +nightly miri test
```

For low-level concurrent data structures, consider `loom` to exhaustively test thread interleavings.

When the project contains `unsafe` code, the Miri run should be part of the project's Makefile.

---

# 11. Errors

Handle errors explicitly with `Result`.

Propagate errors with `?`:

```rust
let contents = std::fs::read_to_string(path)?;
```

In libraries, define typed errors, typically with `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("configuration file not found: {0}")]
    NotFound(PathBuf),
    #[error("invalid configuration")]
    Invalid(#[from] toml::de::Error),
}
```

In applications, `anyhow` is acceptable for adding context at the edges:

```rust
use anyhow::Context;

let config = load(path).context("load configuration")?;
```

Preserve the underlying error as the `source` when callers or logs need it.

Let callers inspect errors by matching on error variants:

```rust
match err {
    ConfigError::NotFound(path) => { /* ... */ }
    ConfigError::Invalid(_) => { /* ... */ }
}
```

or by `downcast_ref` for `anyhow::Error`, instead of comparing error strings.

Add error variants only when callers genuinely need to distinguish them.

Use `Option` rather than an error when absence is a normal, expected outcome.

---

# 12. Panic

Do not panic for normal application errors.

Prefer returning `Result`.

Avoid `unwrap()` in non-test code. Use `expect("why this cannot fail")` only when the invariant is genuinely guaranteed, and make the message state the invariant.

A panic may be appropriate for genuinely unrecoverable programmer errors or impossible states, but it should be uncommon in normal application code.

Do not use `catch_unwind` for normal error handling.

---

# 13. Cancellation and Deadlines

Make cancellation and deadlines explicit.

In async code, futures are cancelled by dropping them. Write async code so that being dropped at any `.await` point leaves state consistent (cancellation safety).

Use explicit mechanisms for cooperative cancellation and timeouts, for example with Tokio:

```rust
use tokio_util::sync::CancellationToken;

async fn run(token: CancellationToken, input: Input) -> Result<(), Error> {
    tokio::select! {
        _ = token.cancelled() => Ok(()),
        result = process(input) => result,
    }
}

let result = tokio::time::timeout(Duration::from_secs(5), fetch()).await;
```

Pass cancellation tokens explicitly to the operations that need them.

Do not use globals or thread-locals to carry request-scoped data or dependencies.

Propagate cancellation to child tasks and operations that support it.

---

# 14. Resource Management

Release resources reliably through ownership and `Drop` (RAII).

Scope resources so they are released as soon as they are no longer needed:

```rust
{
    let mut file = File::create(path)?;
    file.write_all(data)?;
    file.sync_all()?;
} // file is closed here
```

When closing can fail and the failure matters (flushing, committing a transaction), call the explicit method and handle its `Result` rather than relying on `Drop`, which cannot report errors.

Pay particular attention to:

* files
* HTTP response bodies
* database connections
* transactions
* lock guards (do not hold a `MutexGuard` across `.await`)
* timers
* spawned threads and tasks
* subscriptions

Make cleanup behavior obvious.

---

# 15. Concurrency

Use concurrency only when it provides a real benefit.

Do not spawn threads or tasks unnecessarily.

Every spawned thread or task should have a clear:

* owner
* lifecycle
* cancellation strategy
* error-handling strategy

Keep and await `JoinHandle`s, or use structured concurrency (`std::thread::scope`, `tokio::task::JoinSet`), to avoid detached work leaking.

Use channels when communication between concurrent operations is the natural abstraction.

Use `Mutex` / `RwLock` when protecting shared state is simpler.

Do not block inside async code; use `spawn_blocking` for blocking or CPU-heavy work.

Use a single async runtime per application; do not mix runtimes.

---

# 16. Generics

Use generics when they provide meaningful type-safe reuse.

Good candidates include:

* generic algorithms
* reusable data structures
* generic utility functions
* APIs where the type parameter represents a real invariant

Prefer `impl Trait` in argument position for simple cases, and `where` clauses when bounds get long.

Choose between generics (static dispatch) and `dyn Trait` (dynamic dispatch) deliberately; prefer `dyn Trait` when it simplifies the code and performance is not a concern.

Avoid replacing simple code with unnecessarily generic abstractions.

Prefer:

```rust
fn find_user(id: UserId) -> Result<User, Error>
```

over introducing generic abstractions that provide no meaningful benefit.

Generics should reduce duplication or improve type safety, not increase abstraction for its own sake.

---

# 17. Traits

Keep traits small.

Define traits based on the behavior actually required by the consumer.

Prefer:

```rust
pub trait UserRepository {
    fn find(&self, id: UserId) -> Result<Option<User>, RepositoryError>;
}
```

over large traits containing unrelated operations.

Do not create a trait automatically for every struct.

If there is only one implementation and no meaningful abstraction is required, a concrete type may be preferable.

Implement standard traits (`Debug`, `Clone`, `PartialEq`, `Default`, `Display`, `From`, ...) where they are meaningful.

---

# 18. Standard Library First

Prefer the Rust standard library when it provides an adequate solution.

Before adding a dependency, consider:

1. Can the standard library solve the problem?
2. Is the dependency maintained?
3. Does it provide substantial value?
4. What transitive dependencies and features does it introduce?
5. Is the dependency appropriate for the project?

Avoid dependencies for trivial functionality.

Disable default features you do not need.

---

# 19. Dependencies

Use Cargo for dependency management.

Use standard commands:

```bash
cargo add <crate>
cargo remove <crate>
cargo update
cargo tree
cargo tree -i <crate>   # why is this crate included?
```

Review changes to `Cargo.toml` and `Cargo.lock`.

Commit `Cargo.lock`.

Do not manually modify `Cargo.lock`.

Avoid unnecessary dependencies.

---

# 20. Cargo Workspaces

When a project contains multiple crates, use a Cargo workspace.

The workspace acts as the development workspace for the crates, similar conceptually to a .NET solution containing multiple projects.

Example:

```text
project/
├── Cargo.toml
├── Cargo.lock
├── crates/
│   ├── crate-a/
│   │   └── Cargo.toml
│   └── crate-b/
│       └── Cargo.toml
└── application/
    └── Cargo.toml
```

Configure the root `Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/*", "application"]

[workspace.package]
edition = "2024"
rust-version = "1.99"

[workspace.dependencies]
thiserror = "2"
```

Members inherit shared settings:

```toml
[package]
name = "crate-a"
edition.workspace = true
rust-version.workspace = true

[dependencies]
thiserror.workspace = true
```

Declare shared dependency versions in `[workspace.dependencies]`, but each crate must still list every dependency it uses in its own `Cargo.toml`.

There is one `Cargo.lock` and one `target/` directory for the whole workspace.

---

# 21. Crate Independence

A crate should build and test correctly on its own, not only as part of the full workspace.

Cargo unifies features across the packages being built together, which can hide a missing feature in a single crate. Where appropriate, verify crates independently:

```bash
cargo check -p <crate>
cargo test -p <crate>
```

Use crate boundaries to isolate dependencies and compile units when that provides a real benefit.

Do not create a separate crate for every module.

---

# 22. Workspace Maintenance

When adding or removing crates, keep the workspace `members` up to date.

Prefer:

```bash
cargo new --lib crates/new-crate
```

which adds the crate to the workspace automatically when it is not already covered by a `members` glob.

After removing a crate, remove it from `members` and from any dependent `Cargo.toml`, then run:

```bash
cargo check --workspace
```

Keep the workspace clean and representative of the actual development environment.

---

# 23. rust-analyzer

`rust-analyzer` is installed and available in the development environment.

**Use `rust-analyzer` extensively.**

Prefer semantic language-server operations over blind text manipulation whenever possible.

Use `rust-analyzer` for:

* diagnostics
* symbol lookup
* finding references
* finding trait implementations
* type information and inferred types
* navigating definitions
* rename operations
* macro expansion
* understanding module and crate relationships
* identifying affected code before API changes
* discovering usages before refactoring

Before changing a public or widely used API:

1. Find references with `rust-analyzer`.
2. Find trait implementations.
3. Inspect affected crates.
4. Make the change.
5. Check diagnostics again.
6. Run tests.

Do not blindly modify every textual occurrence of a symbol when a semantic operation is available.

Use `rust-analyzer` particularly heavily in multi-crate workspaces.

---

# 24. Formatting

Rust code must be formatted with `rustfmt`.

Use:

```bash
cargo fmt --all
```

Check formatting without modifying files (for CI):

```bash
cargo fmt --all --check
```

Do not manually format Rust code in a style inconsistent with `rustfmt`.

If the project needs non-default settings, put them in `rustfmt.toml`; keep them minimal.

---

# 25. Static Analysis

Use Clippy.

At minimum:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Configure project-wide lint levels in the `[lints]` / `[workspace.lints]` table of `Cargo.toml`, not scattered `#![deny(...)]` attributes.

Enable additional lint groups (such as `clippy::pedantic`) only when the project explicitly adopts them.

Do not add a large collection of lints without a reason.

When suppressing a lint, use `#[expect(clippy::lint_name, reason = "...")]` on the narrowest item.

If another linter or checker (e.g. `cargo deny`, `cargo audit`) is configured for the repository, the Makefile should expose it.

---

# 26. cargo fix

Use `cargo fix` and `cargo clippy --fix` when appropriate for applying compiler and lint suggestions, and `cargo fix --edition` when migrating editions.

Before accepting automated changes:

1. Review the changes.
2. Run tests.
3. Run formatting.
4. Run static analysis.

Do not blindly apply automated transformations to unfamiliar code.

---

# 27. Makefile

Every Rust project MUST have a `Makefile`.

The Makefile is the standard entry point for common development tasks.

At minimum provide:

```text
make help
make format
make format-check
make test
make lint
make build
make doc
make check
make clean
```

Additional targets should be added when useful, for example:

```text
make test-miri
make integration-test
make e2e-test
make benchmark
make audit
```

---

# 28. Makefile Design

Make targets should be:

* predictable
* repeatable
* explicit
* safe
* composable

Use `.PHONY` for command targets.

Example:

```makefile
.PHONY: help format format-check test lint build doc check clean

help:
	@echo "Available targets:"
	@echo "  format        Format Rust code"
	@echo "  format-check  Check formatting"
	@echo "  test          Run tests"
	@echo "  lint          Run clippy"
	@echo "  build         Build the project"
	@echo "  doc           Build documentation"
	@echo "  check         Run validation"
	@echo "  clean         Clean build artifacts"

format:
	cargo fmt --all

format-check:
	cargo fmt --all --check

test:
	cargo test --workspace --all-targets
	cargo test --workspace --doc

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

build:
	cargo build --workspace --all-targets

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

check: format-check lint test build

clean:
	cargo clean
```

`cargo test --all-targets` does not run doc tests; run them separately with `--doc`.

Always pass `--workspace` in multi-crate repositories so every member is covered.

---

# 29. Makefile as Developer Interface

For common operations, use the Makefile.

Prefer:

```bash
make test
```

over requiring developers to remember the exact underlying command.

Prefer:

```bash
make build
```

over undocumented build commands.

The underlying Cargo commands should still remain straightforward and discoverable.

---

# 30. CI Compatibility

Make targets should work in CI.

Avoid targets that depend on:

* a developer's shell aliases
* interactive input
* local editor configuration
* undocumented environment state

CI should be able to run the same commands developers run locally.

In CI, pass `--locked` to Cargo commands so builds fail if `Cargo.lock` is out of date.

A good CI pipeline should be able to invoke:

```bash
make check
```

plus project-specific integration tests.

---

# 31. Build

Use standard Cargo builds.

Examples:

```bash
cargo build --workspace
cargo build --release
```

Configure build profiles in the workspace root `Cargo.toml` (`[profile.release]`), not via ad-hoc flags.

For applications, provide explicit Makefile targets where useful.

Build failures must propagate to `make`.

Do not hide compiler errors or convert failed commands into successful Make targets.

Do not silence compiler warnings with crate-wide `#![allow(warnings)]`.

---

# 32. Code Quality Workflow

For normal development, use:

```text
write test
    ↓
implement
    ↓
cargo fmt
    ↓
run focused tests (cargo test -p <crate> <filter>)
    ↓
run broader tests
    ↓
cargo clippy
    ↓
Miri when unsafe code is involved
    ↓
make check
```

Use `cargo check` for fast feedback while iterating.

Do not wait until the end of a large feature to discover basic formatting, compilation, or test failures.

---

# 33. Refactoring

Refactor continuously when tests provide sufficient safety.

Prefer small changes.

Examples:

* simplify functions
* improve names
* remove duplication
* reduce unnecessary dependencies
* reduce unnecessary cloning and allocation
* improve error handling
* simplify APIs
* eliminate dead code

Use `rust-analyzer` when performing semantic refactoring.

Do not perform large rewrites without a concrete reason.

---

# 34. Performance

Do not optimize without evidence.

Measure release builds, never debug builds.

When performance matters, use benchmarks and profilers:

```bash
cargo bench
```

Prefer `criterion` (or `divan`) for benchmarks on stable Rust.

Use CPU and memory profiling (`perf`, `cargo flamegraph`, heap profilers) when appropriate.

Prefer algorithmic improvements over micro-optimizations.

Keep performance-related tests or benchmarks when they protect an important requirement.

---

# 35. Security

Follow secure coding practices.

Never log:

* passwords
* authentication tokens
* private keys
* credentials
* secrets

Avoid deriving `Debug` on types holding secrets, or redact them (e.g. with the `secrecy` crate).

Validate untrusted input.

Avoid:

* command injection
* SQL injection
* path traversal
* unsafe deserialization
* insecure randomness
* integer overflow in security-relevant arithmetic (use `checked_*` operations)
* accidental secret exposure

Avoid `unsafe`. When it is truly necessary:

* keep it in small, well-encapsulated modules behind a safe API
* document every `unsafe` block with a `// SAFETY:` comment explaining why it is sound
* cover it with tests run under Miri

Consider `#![forbid(unsafe_code)]` in crates that do not need it.

Use established, audited cryptography crates rather than implementing cryptography yourself.

Use `cargo audit` or `cargo deny` to check dependencies for known vulnerabilities when the project adopts them.

---

# 36. Development Completion Checklist

Before considering a code change complete:

### Code

* Code is idiomatic Rust.
* Code is formatted.
* Errors are handled with `Result`; no stray `unwrap()`.
* Cancellation and timeouts are handled appropriately.
* Resources are cleaned up.
* No unnecessary abstraction was introduced.
* Generics are used only where beneficial.
* Any `unsafe` code is justified and documented.

### Tests

* New behavior has tests.
* TDD was followed where practical.
* Tests are deterministic.
* Relevant integration tests exist.
* Miri is run when `unsafe` code is involved.

### Dependencies

* Dependencies are justified.
* `Cargo.toml` is correct.
* `Cargo.lock` is updated and committed.
* Workspace `members` are updated when applicable.

### Tooling

* `rust-analyzer` diagnostics are clean or understood.
* `cargo clippy` passes with no warnings.
* Formatting passes.
* Build succeeds.
* Makefile targets work.

### Final validation

Run:

```bash
make check
```

and any additional project-specific test targets.

---

# 37. General Rule

Prefer:

```text
idiomatic Rust
+
simple code
+
TDD
+
built-in test harness
+
rustfmt
+
rust-analyzer
+
clippy
+
Cargo
+
Cargo workspaces for multi-crate development
+
Makefile
+
repeatable development workflows
```

Avoid:

```text
unnecessary abstractions
+
unnecessary generics
+
unnecessary dependencies
+
large traits
+
unwrap() in production code
+
unjustified unsafe
+
blind text-based refactoring
+
tests added only after implementation
+
manual repetitive development commands
```
