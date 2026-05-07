# Abu Code

A coding agent CLI built with the [abu-agent](https://github.com/MoleSir/abu) framework. Think of it as a lightweight, hackable alternative to Claude Code.

## Features

- **Conversational coding** — natural language to code changes, with full context
- **18 built-in tools** — Bash, Read, Write, Edit, Glob, Grep, and more
- **Task system** — mandatory task tracking with dependency chains (blocked_by)
- **Persistent memory** — remembers preferences, feedback, and project facts across sessions
- **Session resume** — conversation history saved as JSONL, auto-resume on restart
- **3 subagent types** — `task` (read/write), `explore` (read-only search), `plan` (architecture design)
- **Background tasks** — long-running commands execute asynchronously with completion notifications
- **Permission system** — 3 modes (Auto / Plan / Default), deny-list for dangerous commands
- **Incremental context compaction** — only summarizes new messages, token-aware thresholds
- **Skills** — load domain knowledge from `./skills/` directory
- **MCP support** — connect external tool servers via `.mcp.json`
- **CLAUDE.md** — loads user-global (`~/.claude/CLAUDE.md`) and project-level instructions

## Installation

```bash
git clone https://github.com/MoleSir/abu.git
cd abu/examples/abu-code
cargo build --release
```

## Quick Start

Create a `.env` file:

```env
CHAT_MODEL=deepseek-chat
DEEPSEEK_API_KEY=sk-your-key-here
```

Run:

```bash
cargo run
```

```
Abu Code  |  Model: deepseek-chat
Project:   "/home/you/project"
Data dir:  "~/.abu-code/projects/home-you-project-a1b2c3d4"
Mode:      Auto
Type /help for commands.
>
```

## Configuration

| Env Var | Default | Description |
|----------|---------|-------------|
| `CHAT_MODEL` | `deepseek-chat` | Model name |
| `CHAT_MODEL_COMPACT` | same as `CHAT_MODEL` | Model for context summarization |
| `DEEPSEEK_BASE_URL` | `https://api.deepseek.com` | API endpoint (works with any OpenAI-compatible API) |
| `DEEPSEEK_API_KEY` | — | API key |

To use OpenAI:

```env
CHAT_MODEL=gpt-4o
DEEPSEEK_BASE_URL=https://api.openai.com/v1
DEEPSEEK_API_KEY=sk-openai-key
```

To use a local model:

```env
CHAT_MODEL=llama-3-70b
DEEPSEEK_BASE_URL=http://localhost:8080/v1
DEEPSEEK_API_KEY=not-needed
```

## Commands

| Command | Description |
|---------|-------------|
| `/help` | Show all commands |
| `/tools` | List all registered tools |
| `/mode` | Show current permission mode |
| `/plan` | Switch to Plan mode (read-only) |
| `/auto` | Switch to Auto mode (safe auto-approved) |
| `/default` | Switch to Default mode (ask for everything) |
| `/memory` | List saved memories |
| `/tasks` | List current tasks |
| `/sessions` | List saved sessions |
| `/clear` | Start a fresh session |
| `/save` | Manually save current session |
| `/quit` | Exit (auto-saves) |

## Tools

### File Operations
- **Bash** — execute shell commands (120s timeout, dangerous command blocklist)
- **ReadFile** — read file contents
- **WriteFile** — write content to files (creates parent directories)
- **EditFile** — exact string replacement with optional `replace_all`

### Code Exploration
- **Glob** — find files by pattern (`**/*.rs` for recursive)
- **Grep** — search file contents with regex, optional file filter

### Task Management
- **task_create** — create a task with optional `blocked_by` dependencies
- **task_update** — update status, add dependencies
- **task_list** — list all tasks with status and blockers
- **task_get** — get full task details

### Memory
- **save_memory** — persist a fact across sessions (user/feedback/project/reference)

### Background Tasks
- **background_run** — run a long command asynchronously
- **background_check** — check a task's status and output
- **background_list** — list all background tasks

### Subagents
- **task** — general-purpose (read, write, edit, execute)
- **explore** — read-only code explorer (Glob, Grep, Read)
- **plan** — architecture designer (read-only, produces step-by-step plans)

## Data Storage

Everything lives under `~/.abu-code/projects/<path-slug>-<hash>/`:

```
~/.abu-code/projects/home-you-project-a1b2c3d4/
├── memory/           # Persistent memories (Markdown + frontmatter)
├── tasks/            # Task files (JSON)
├── sessions/         # Conversation history (JSONL)
├── background/       # Background task logs
└── tool_results/     # Cached large tool outputs
```

## Project Structure

```
abu-code/
├── Cargo.toml
└── src/
    ├── main.rs          # Entry point, REPL, agent assembly
    ├── config.rs        # Model configuration from env vars
    ├── system_prompt.rs # Dynamic system prompt + CLAUDE.md loading
    ├── tools.rs         # Bash, Read, Write, Edit, Glob, Grep
    ├── task.rs          # Task system with dependencies + middleware
    ├── memory.rs        # Persistent memory system
    ├── session.rs       # Session save/load as JSONL
    ├── background.rs    # Async background task execution
    ├── compact.rs       # Incremental context summarization
    ├── permission.rs    # Permission manager + user authorization
    ├── hook.rs          # Terminal output formatting
    └── subagent.rs      # Task, Explore, Plan subagent factories
```

## License

MIT — see the root [LICENSE](../../LICENSE) file.
