# envbroker

> [!NOTE]
> Are you worried your agent might do stupid stuff with your precious `.env` variables?  
> Do you handle high-risk secrets like wallet keys, API tokens, or service credentials in your `.env` file while *vibe-coding* ?

`envbroker` is a CLI for guarding secret variables that usually live in `.env` files, such as `API_KEY`, `SECRET_KEY`, database URLs, and access tokens, while still making them available to approved commands.

It is built for agentic coding workflows, especially high-autonomy or YOLO-style runs where an agent can move quickly and touch a lot of files and commands. Instead of relying on a fancy sandbox, `envbroker` uses a simple approach that works in practice: encrypt the real `.env`, store it outside the repository, replace the in-repo file with placeholders, and use Claude Code hooks to steer secret-dependent commands through `envbroker run`.

> btw this cli is vibe coded also lol

## Status

The current implementation focuses on:

- Claude Code integration
- `age` encryption for secret payloads
- OS keychain storage for the decryption identity
- Git-repository workflows with placeholder `.env` files

## Demo

[![envbroker demo](https://img.youtube.com/vi/wkU4WlWLF88/maxresdefault.jpg)](https://youtu.be/wkU4WlWLF88)

## Installation

```sh
cargo install envbroker
```

## Quick Start

1. Create a normal `.env` in a git repository.
2. Install Claude Code integration.
3. Run your secret-dependent commands through `envbroker run`.

```sh
envbroker install claude
envbroker status
envbroker list-vars
envbroker run -- cargo test
```

After installation, the original `.env` is rewritten to placeholders like this:

```dotenv
# Managed by envbroker. Real values are encrypted outside this repository.
# ENVBROKER_ACTIVE
OPENAI_API_KEY=ENVBROKER_REQUIRED
DATABASE_URL=ENVBROKER_REQUIRED
```

## How It Works

1. `envbroker install claude` parses your `.env`, encrypts it with `age`, stores the identity in the OS keychain, and writes ciphertext outside the repository.
2. `.env` is replaced with `ENVBROKER_REQUIRED` placeholders.
3. Claude Code hooks are installed:
   - **PreToolUse** blocks direct `.env` reads (`cat .env`, etc.) and prompts for approval on `envbroker run` commands.
   - **PostToolUseFailure** detects when a command fails due to placeholder values and guides Claude to retry through `envbroker run -- ...`.
4. You just prompt Claude normally. The hooks handle secret access automatically — no need to mention `.env` or `envbroker` in your prompt.

## Command Reference

```text
envbroker install claude [--scope <local|project|user>] [--env-file <path>] [--profile <name>]
envbroker uninstall claude [--scope <local|project|user>]
envbroker run [--profile <name>] -- <command>...
envbroker status
envbroker doctor
envbroker list-vars [--profile <name>]
```

Useful examples:

```sh
envbroker install claude --scope local --env-file .env --profile default
envbroker run -- cargo run
envbroker run -- npm test
envbroker doctor
envbroker uninstall claude
```

## Files and Data

In the repository:

- `.env` becomes a placeholder file
- `.envbroker/config.json` stores repo-local metadata
- `.claude/hooks/envbroker-pretooluse` and `.claude/hooks/envbroker-posttoolusefailure` are created
- Claude settings are updated with a deny rule for `Read(./.env)` and envbroker hook entries

Outside the repository:

- encrypted secrets are stored under the platform app-data directory for `envbroker`
- project metadata is stored alongside the encrypted payload
- the decryption identity is stored in the OS keychain under the `envbroker` service

## Caveats

- Run `envbroker` inside a git repository. Project discovery walks upward until it finds `.git`.
- Current agent installation flow is Claude-specific.
- The repository code currently uses the Apple Keychain backend for `keyring`.

## Development

```sh
cargo fmt
cargo test
cargo run -- --help
```

## License

MIT. See [LICENSE](LICENSE).
