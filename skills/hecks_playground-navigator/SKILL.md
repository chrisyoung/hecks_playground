---
name: hecks_playground-navigator
description: 'Navigate the HecksPlayground domain compiler framework. Use when exploring the codebase, finding where things live, understanding the compiler pipeline, or figuring out how modules connect. Covers the bluebook DSL, hecksagon runtime, generators, CLI, workshop, AI tools, and examples.'
license: MIT
metadata:
  author: hecks_playground
  version: "1.0.0"
---

# HecksPlayground Codebase Navigator

HecksPlayground is a domain compiler: you write a domain model in a Ruby DSL (Bluebook), it builds an in-memory IR (Hecksagon), and generators emit runnable code for multiple targets (Ruby, Go).

## Top-Level Layout

| Directory | Purpose |
|-----------|---------|
| `bluebook/` | DSL parser — the source-of-truth grammar for domain models |
| `hecksagon/` | Runtime IR — aggregates, value objects, commands, events, policies as live objects |
| `hecksties/` | Glue layer — `HecksPlayground.boot`, `HecksPlayground.domain`, CLI, multi-domain support |
| `hecks_playground_targets/` | Code generators — `ruby/` and `go/` static targets |
| `hecks_playground_workshop/` | Interactive workshop UI (web explorer for domains) |
| `hecks_playground_ai/` | AI/MCP tools — domain modeling via tool calls, watcher agent |
| `hecks_playground_on_rails/` | Rails integration — `HecksPlayground.configure`, HecksLive/Turbo |
| `lib/hecks_playground.rb` | Meta-gem loader — adds all sub-gem lib/ dirs to `$LOAD_PATH` |
| `examples/` | Working example apps (pizzas, banking, multi_domain, rails, sinatra) |
| `docs/` | Usage documentation, one file per feature |
| `bin/` | CLI entrypoint |

## The Compiler Pipeline

```
Bluebook DSL  →  Builders  →  Hecksagon IR  →  Generators  →  Ruby/Go code
     ↓                            ↓
  bluebook/lib/          hecksagon/lib/
  bluebook/lib/hecks_playground/    hecksagon/lib/hecksagon/
```

1. **Write**: User defines domain in Bluebook DSL (`HecksPlayground.domain "Pizzas" { ... }`)
2. **Parse**: `bluebook/lib/bluebook/` builders construct IR nodes
3. **Build**: `hecksagon/lib/hecksagon/structure/` holds the IR types (Aggregate, ValueObject, Command, Event, etc.)
4. **Validate**: `hecksagon/lib/hecksagon/contract_validator.rb` runs data contracts for cross-target consistency
5. **Generate**: `hecks_playground_targets/hecks_playground_static/` or `hecks_playground_targets/go_hecks_playground/` emit static code from the IR

## Key Modules

### Bluebook (DSL)
- `bluebook/lib/bluebook.rb` — entry point
- `bluebook/lib/bluebook/` — DSL blocks, builders, validators
- `bluebook/lib/hecks_playground/` — `HecksPlayground.domain` definition hook

### Hecksagon (Runtime IR)
- `hecksagon/lib/hecksagon.rb` — entry point
- `hecksagon/lib/hecksagon/structure/` — IR node types (Aggregate, Command, Event, Policy, ValueObject, Entity, etc.)
- `hecksagon/lib/hecksagon/dsl/` — runtime DSL for extending domains
- `hecksagon/lib/hecksagon/contract_validator.rb` — 8 data contracts
- `hecksagon/lib/hecksagon/domain_mixin.rb` — mixed into domain modules
- `hecksagon/lib/hecksagon/adapter_registry.rb` — persistence adapter registry
- `hecksagon/lib/hecksagon/acl_builder.rb` — anti-corruption layer builder

### Hecksties (Glue / CLI)
- `hecksties/lib/hecks_playground.rb` — `HecksPlayground.boot`, `HecksPlayground.configure`, `HecksPlayground.hecksagon`
- `hecksties/lib/hecks_playground_cli.rb` — CLI framework
- `hecksties/lib/hecks_playground_cli/` — individual CLI commands (one file each)
- `hecksties/lib/hecks_playground_multidomain.rb` — multi-domain orchestration, FilteredEventBus

### Targets (Code Generation)
- `hecks_playground_targets/hecks_playground_static/` — static Ruby target generator
- `hecks_playground_targets/go_hecks_playground/` — static Go target generator
- Generators use `HecksTemplating::Names` for naming conventions

### Workshop (Interactive UI)
- `hecks_playground_workshop/lib/` — web-based domain explorer
- `hecks_playground_workshop/explorer/` — frontend assets

### AI Tools
- `hecks_playground_ai/lib/hecks_playground_ai.rb` — entry point
- `hecks_playground_ai/lib/hecks_playground_ai/mcp_server.rb` — MCP server for AI domain modeling
- `hecks_playground_ai/lib/hecks_playground_ai/aggregate_tools.rb` — aggregate CRUD via MCP
- `hecks_playground_ai/lib/hecks_playground_ai/build_tools.rb` — build/compile tools
- `hecks_playground_ai/lib/hecks_playground_ai/play_tools.rb` — play mode (execute commands live)
- `hecks_playground_ai/lib/hecks_playground_ai/session_tools.rb` — session management
- `hecks_playground_ai/lib/hecks_playground_ai/inspect_tools.rb` — domain inspection
- `hecks_playground_ai/watcher_agent/` — file watcher agent

### Persistence
- `hecksagon/lib/hecks_playground_persist.rb` — persistence layer
- `hecksagon/lib/hecks_playground_persist/` — adapters (memory, SQL, migrations)

## Examples — Quick Reference

| Example | What it demonstrates |
|---------|---------------------|
| `examples/pizzas/` | Canonical single-domain app — aggregates, commands, events |
| `examples/banking/` | Lifecycle/state machines, domain services |
| `examples/multi_domain/` | Cross-domain events, FilteredEventBus, subscribe DSL |
| `examples/pizzas_rails/` | Rails integration with `HecksPlayground.configure` |
| `examples/governance/` | Policy-heavy domain, specifications |
| `examples/sinatra_app/` | Sinatra integration |

## Entry Points

- **Boot an app**: `HecksPlayground.boot(__dir__)` — loads `bluebook/` subfolder, builds IR
- **Rails config**: `HecksPlayground.configure { domain "Pizzas", path: "..." }`
- **Run examples**: `ruby -Ilib examples/pizzas/pizzas.rb`
- **CLI**: `bin/hecks_playground` or `bundle exec hecks_playground`
- **Specs**: `bundle exec rspec` (must run under 1 second)

## Conventions

- Aggregates are pure domain objects — no persistence logic
- Memory adapters for tests — fast and isolated
- CalVer versioning (YYYY.MM.DD.N)
- Module grouping: parent file with `.bind`, children in subdirectory
- CLI commands: one file each under `hecksties/lib/hecks_playground_cli/commands/`
- Generators show a diff when a target file already exists (never silently overwrite)
- 200-line limit per lib file (doc headers excluded)
