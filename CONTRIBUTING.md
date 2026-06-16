# Contributing to rust365

Thanks for your interest in improving rust365!

## Building

```
cargo build --release
```

Produces `target/release/rust365`.

## Testing

```
cargo test --release
```

The smoke test converts the bundled `tests/sample.docx` fixture and checks the
generated HTML.

## Guidelines

- Format with `cargo fmt` (rustfmt defaults).
- Keep the tool **dependency-free at runtime** — please don't add runtime crates.
- Keep changes focused; one logical change per pull request.
- If you change behavior, describe it in the PR (and update the README if it is
  user-facing).

## Reporting issues

Open an issue with the exact command you ran and, if possible, a minimal `.docx`
that reproduces the problem.
