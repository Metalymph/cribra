# cribra-cli

Canonical command-line interface for Cribra.

`cribra-cli` contains both:

- a reusable Rust command library;
- the thin standalone `cribra` executable.

Detection, validation, findings, candidates, remediation, and transformations
remain authoritative in the `cribra` core crate.

## Install

```text
cargo install cribra-cli
```

## Usage

```text
cribra scan <FILE>
cribra scan <FILE> --format json
cribra scan -
```

The CLI accepts exactly one explicit UTF-8 file or stdin.

It does not perform directory traversal, repository walking, glob expansion, or
automatic encoding conversion in v0.4.4.

## Output

Human-readable output is the default.

```text
cribra scan config.env
```

Machine-readable metadata:

```text
cribra scan config.env --format json
```

Both forms are metadata-only and do not intentionally render matched secret
values.

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | successful execution, including clean, review-only, and finding-bearing scans |
| `1` | input or execution failure |
| `2` | command-line usage or parse failure |

## Reusable Rust API

The library exposes a reusable command model and execution surface so a Rust
consumer can invoke canonical Cribra command semantics without spawning the
binary.

Core public types include:

- `Command`;
- `ScanCommand`;
- `ScanInput`;
- `OutputFormat`;
- `ParseError`;
- `ExecuteError`;
- `CommandOutput`.

Core entry points include command parsing, execution, and the process-style
`run` adapter.

## Architecture

```text
standalone `cribra`
        │
        ▼
    cribra-cli
    ├── parser
    ├── command model
    ├── input acquisition
    ├── execution
    └── rendering
        │
        ▼
      cribra
      └── semantic authority
```

The CLI does not duplicate detector implementation.

## Privacy

- invalid UTF-8 is rejected;
- source contents are not included in normal diagnostics;
- matched secret values are not included in finding output;
- review candidates remain metadata-only;
- no network access is introduced by the CLI.

## License

MIT
