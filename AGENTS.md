# AGENTS.md — Contributing Guidelines

Guidelines for AI agents and human contributors working on this codebase.

## Before You Code

1. **Read the docs** — `README.md`, `LLM_USAGE.md`, and relevant source files
2. **Understand the architecture** — This is a thin CLI wrapper around [`account_sdk`](https://github.com/cartridge-gg/controller-rs)
3. **Check existing patterns** — Follow the style of existing commands in `src/commands/`

## Code Style

### Rust Standards

- **Format**: Run `cargo fmt` before committing
- **Lint**: Run `cargo clippy` and fix warnings
- **Build**: Ensure `cargo build` succeeds
- **Test**: Run `cargo test` if tests exist

### CLI Conventions

- All commands support `--json` for machine-readable output
- Use the `JsonOutput` struct for consistent response format
- Include `error_code`, `message`, and `recovery_hint` in error responses
- Add new commands to `src/commands/mod.rs`

### Documentation

- Update `LLM_USAGE.md` when adding/modifying commands
- Update `SKILL.md` when adding new tools for agents
- Include examples in doc comments

## PR Guidelines

### Before Opening a PR

```bash
# Required checks
cargo fmt
cargo clippy
cargo build
cargo test  # if tests exist
```

### PR Title Format

```
type(scope): description

# Examples:
feat(marketplace): add buy and info commands
fix(session): handle expired token refresh
docs(readme): add calldata format examples
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`

### PR Description

Include:
- **What** — Brief description of changes
- **Why** — Motivation or issue being solved
- **How** — Technical approach if non-obvious
- **Testing** — How you verified the changes work

### Commit Messages

- Use conventional commits format
- Keep subject line under 72 characters
- Reference issues with `#123` if applicable

## Adding New Commands

1. Create a new file in `src/commands/` (or a subdirectory for command groups)
2. Implement the command struct with clap derive macros
3. Add to the command enum in `src/commands/mod.rs`
4. Add the subcommand variant in `src/main.rs`
5. Update `LLM_USAGE.md` with usage examples
6. Update `.claude/skills/controller-skill/skill.md` if it's agent-relevant

### Command Structure

```rust
use clap::Parser;
use crate::utils::output::JsonOutput;

#[derive(Parser, Debug)]
pub struct MyCommand {
    /// Description of the argument
    #[arg(long)]
    pub some_arg: String,
}

impl MyCommand {
    pub async fn run(&self, config: &Config) -> Result<()> {
        // Implementation
    }
}
```

## Security Considerations

- Never log or output private keys
- Session credentials stay in `~/.config/controller-cli/`
- Validate all user inputs
- Use `--json` output for programmatic access (no parsing stdout text)

## Getting Help

- [Controller Docs](https://docs.cartridge.gg/controller)
- [Starknet Docs](https://docs.starknet.io/)
- [account_sdk source](https://github.com/cartridge-gg/controller-rs)
