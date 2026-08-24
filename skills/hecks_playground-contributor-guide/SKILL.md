---
name: hecks_playground-contributor-guide
description: 'Developer workflow for contributing to the HecksPlayground framework. Use before implementing any feature, fix, or refactor. Covers file checklists for every type of change, naming conventions, pre-commit hooks, and the full architecture map. Read CONTRIBUTING.md for the exhaustive guide.'
license: MIT
metadata:
  author: hecks_playground
  version: "1.0.0"
---

# HecksPlayground Contributor Guide

Before writing any code in this codebase, consult `CONTRIBUTING.md` at the project root. It contains exhaustive how-to guides for every type of change.

## Quick File Checklist

| Change | Files to update |
|---|---|
| New DSL keyword | IR struct, builder, HANDLE_METHODS, generators (Ruby + Go), validator, specs, FEATURES.md, docs/usage/ |
| New CLI command | `hecksties/lib/hecks_playground_cli/commands/<name>.rb`, spec |
| New extension | `hecksties/lib/hecks_playground/extensions/<name>.rb`, spec, FEATURES.md |
| New runtime method | `hecksties/lib/hecks_playground/runtime.rb`, spec, FEATURES.md, docs/usage/ |
| New autoloaded class | New file, autoload in `hecksties/lib/hecks_playground/autoloads.rb`, spec |
| New data contract | `hecksties/lib/hecks_playground/conventions/<name>_contract.rb`, update templates, spec |
| New validation rule | `bluebook/lib/hecks_playground/validation_rules/`, register in validator, spec |
| New example app | `examples/<name>/`, Bluebook + app.rb + Hecksagon (if needed) |
| Rename/restructure | See `skills/hecks_playground-rename-playbook/SKILL.md` |

## Before Every Commit

1. Update `FEATURES.md` if new features were added
2. Add `docs/usage/<feature>.md` with runnable examples
3. `bundle exec rspec` — all must pass under 1 second
4. `find lib -name "*.rb" -exec wc -l {} + | sort -rn | head -5` — no file over 200 code lines
5. `ruby -Ilib examples/pizzas/pizzas.rb` — smoke test

## Naming — Never Inline String Transforms

| Need | Use |
|---|---|
| PascalCase | `HecksPlayground::Utils.sanitize_constant(name)` |
| snake_case | `HecksPlayground::Utils.underscore(name)` |
| Human readable | `HecksPlayground::Utils.humanize(name)` |
| Short class name | `HecksPlayground::Utils.const_short_name(obj)` |
| Domain module name | `HecksPlayground::Conventions::Names.domain_module_name(name)` |
| Domain gem name | `HecksPlayground::Conventions::Names.domain_gem_name(name)` |
| Aggregate slug | `HecksPlayground::Conventions::Names.domain_aggregate_slug(name)` |

## Architecture (Quick Reference)

```
bluebook/       — DSL grammar, IR types, builders, validators, generators
hecksagon/      — Hexagonal infra: ACL, adapters, gates
hecksties/      — Core kernel: autoloads, utils, runtime, extensions, CLI, conventions
hecks_playground_targets/  — Code generators (ruby/, go/)
hecks_playground_workshop/ — Interactive REPL, web explorer
hecks_playground_ai/       — MCP server, AI tools
examples/       — Working apps (pizzas, banking, multi_domain, governance)
```

## Key Files by Task

### Adding a DSL concept
1. `bluebook/lib/hecks_playground/domain_model/structure/` or `behavior/` — IR node
2. `bluebook/lib/hecks_playground/dsl/aggregate_builder.rb` — DSL method
3. `bluebook/lib/hecks_playground/generators/domain/` — Ruby generator
4. `hecks_playground_targets/hecks_playground_static/` and `hecks_playground_targets/go_hecks_playground/` — static targets
5. `hecksties/lib/hecks_playground/conventions/` — data contract (if cross-target)

### Modifying the command lifecycle
- Pipeline: `hecksties/lib/hecks_playground/mixins/command/lifecycle_steps.rb`
- Step methods: `hecksties/lib/hecks_playground/mixins/command.rb`
- Alternative: use middleware via `runtime.use` (no pipeline change needed)

### Adding persistence
- Interface: `find`, `all`, `count`, `save`, `delete`, `where`, `clear`
- Register via `HecksPlayground.register_extension(:name)`
- Users activate with `HecksPlayground.boot(__dir__, adapter: :name)`

## Rules

- Always work on feature branches, never commit directly to main
- Cross-component requires use `require`, not `require_relative`
- No backward compatibility shims — break APIs freely (no users yet)
- Generators show a diff when target file exists (never silently overwrite)
- Read `CONTRIBUTING.md` for the full guide with code examples
