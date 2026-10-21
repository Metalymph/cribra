# cribra-cli

Canonical command-line interface for Cribra.

`cribra-cli` provides the `cribra` executable and a reusable Rust command
library. Detection, validation, findings, candidates, remediation,
explainability, and transformations remain authoritative in the `cribra` core
crate.

See [`docs/CLI_PARITY.md`](../../docs/CLI_PARITY.md) for the complete command
contract, output details, and parity matrix.

## Install

```text
cargo install cribra-cli
```

## Commands

```text
cribra scan <FILE|->
cribra redact <FILE|->
cribra template <FILE|->
cribra pseudonymize <FILE|-> --key-file <PATH>
cribra synthesize <FILE|-> --key-file <PATH>
cribra bundle --mode <MODE> [--key-file <PATH>] --output <DIR> <FILE>...
```

Single-source commands accept exactly one explicit UTF-8 file or `-` for stdin.
The CLI does not traverse directories, walk repositories, expand globs, or
perform automatic encoding conversion.

### Scan

```text
cribra scan config.env
cribra scan config.env --format json
```

`scan` supports exact and minimum filters with `--severity` and
`--min-severity`, exact and minimum confidence filters with `--confidence` and
`--min-confidence`, and `--rule-id`. Findings can be sorted with `--sort` by
rule ID, severity, confidence, or location, in ascending or descending order.

`--explain` adds the detection mode resolved by the core for each rendered
finding. Human-readable output is the default; `--format json` produces
deterministic metadata. Finding output does not intentionally include matched
secret values.

### Redaction and templating

```text
cribra redact source.env
cribra template source.env
```

`redact` replaces classified findings with `[REDACTED]`. `template` replaces
each finding with `<CRIBRA:rule-id>` and rejects overlapping findings. Both
commands write only the transformed UTF-8 source to stdout; text outside
finding spans is preserved, including BOMs, CRLF, and trailing-newline state.

Candidates are not transformed, so output may still contain sensitive data.

### Pseudonymization and synthesis

```text
cribra pseudonymize source.env --key-file key.bin
cribra synthesize source.env --key-file key.bin
```

Both keyed commands require a regular key file containing exactly 32 raw binary
bytes. Symlinks to regular files are accepted; hex, Base64, whitespace, and
newlines are not decoded or trimmed. Key bytes are read and validated before
the source, are not printed, and are not guaranteed to be zeroized in memory.

Reusing a key makes pseudonymization reproducible and enables correlation
across runs and datasets. Synthesis is deterministic for the key, rule ID, and
source span. Manage keys outside the CLI, restrict access, and use separate keys
where cross-dataset correlation is unwanted.

### Bundle export

```text
cribra bundle --mode redact --output bundle sources...
cribra bundle --mode pseudonymize --key-file key.bin --output bundle sources...
```

Bundle export is supported on macOS and Linux. Modes are `redact`, `template`,
`pseudonymize`, and `synthesize`; keyed modes require the same 32-byte key file.
The command accepts multiple explicit source files, rejects stdin, and writes:

```text
bundle/
  manifest.json
  sources/
    0001.txt
    0002.txt
    ...
```

The output parent must already exist, and the destination must not exist.
Staging and output files use private permissions. Publication uses atomic
no-replace rename and never overwrites an existing destination. Candidate and
undetected sensitive values may remain in exported sources.

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | successful execution, including clean, review-only, and finding-bearing scans |
| `1` | key, input, transformation, bundle validation/export, unsupported platform, or other execution failure |
| `2` | command-line usage or parse failure |

Finding presence alone does not cause a non-zero exit.

## Privacy limitations

- Scan output is metadata-only and does not intentionally render matched secret
  values.
- `redact`, `template`, `pseudonymize`, and `synthesize` emit transformed source
  as their result. Candidates, undetected values, and key material present in
  unchanged text may remain.
- Key reuse enables correlation. Pseudonymized or synthesized output is not a
  guarantee of anonymity or secret-free content.
- Invalid UTF-8 is rejected. Diagnostics avoid source values, transformed
  content, and key material.
- The CLI introduces no network access.

## Reusable Rust API

The library exposes the command model, parser, execution surface, and rendering
used by the executable, so Rust consumers can invoke canonical Cribra command
semantics without spawning the binary.

Core public types include `Command`, `ScanCommand`, `ScanInput`, `OutputFormat`,
`ParseError`, `ExecuteError`, and `CommandOutput`. Entry points include command
parsing, execution, and the process-style `run` adapter.

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

## License

MIT
