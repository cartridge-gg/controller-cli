---
name: controller-cli-development
description: Contributor workflow for cartridge-gg/controller-cli. Use when implementing or reviewing CLI command behavior, output contracts, install/release scripts, and Rust quality checks in this repository.
---

# Controller CLI Development

Use this skill to build and validate changes in `cartridge-gg/controller-cli`.

## Core Workflow

1. Build and run locally:
   - `cargo build`
   - `cargo run -- <command>`
2. Run standard validation gates:
   - `cargo fmt`
   - `cargo clippy -- -D warnings`
   - `cargo test`
3. Use consolidated Make targets when preferred:
   - `make build`
   - `make test`
   - `make lint`
   - `make check`

## Behavioral Validation

- Verify JSON output compatibility when changing CLI responses (`--json`).
- Re-test register/execute/status flows when session/auth code is touched.
- Keep `LLM_USAGE.md` and command examples aligned with behavior changes.

## PR Checklist

- Document command-level behavior changes and migration notes.
- Include command outputs or structured examples for modified flags.
- Report the exact validation commands that passed.
