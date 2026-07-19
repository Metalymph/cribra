# Contributing

Silens Scan is in its initial design and implementation stage.

Before opening a substantial pull request, open an issue describing:

- the proposed behavior
- public API impact
- privacy and portability implications
- false-positive and false-negative tradeoffs
- test coverage

## Local validation

```bash
cargo fmt --all --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
git diff --check
```
