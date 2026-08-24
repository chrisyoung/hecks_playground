# tooling/

Here live the small instruments the framework uses on itself. Not part of the language, not part of an integration ; the workshop's own tools, *comme les outils sur l'établi* — kept separate from the work so neither gets in the other's way.

**hecksagain-cutover note (i768 slice 5.1):** `features_audit.rb` and `verify` were deleted as DEAD — both referenced the pre-cutover `ruby/`-based `hecks_playground` gem via a top-level `lib/` path that no longer resolves (they predate this migration and were unreferenced by anything else). `tool_cache/` and `install-hooks` stay : `tool_cache/` is real, live dependency of `bin/storehouse-tools` and `bin/update-tool-cache` ; `install-hooks` is the installer for `git-hooks/`, which stays per the cutover PRD's Goal 1.

## What lives here

| File / dir              | What it is |
|------------------------|---|
| `git-hooks/`           | The tracked git hooks : `pre-commit` (parity gate, lifecycle validator, antibody surface) and `commit-msg` (antibody enforcement — requires bluebook-only files or explicit exemption markers in commit messages) |
| `install-hooks`        | One-shot installer that copies `git-hooks/*` into `.git/hooks/`. Run after clone, or whenever a hook changes |
| `tool_cache/`          | `snapshot_writer.rb` + `transcript_walker.rb`, `require_relative`'d by `bin/storehouse-tools` and `bin/update-tool-cache` — the MCP tool-cache roster mechanism |
| `storehouse-mcp/`      | The MCP-door compatibility shim (stays in `hecks_playground` per the hecksagain-cutover PRD's Goal 1) |

## Installing hooks

```sh
tooling/install-hooks
```

Copies every file in `tooling/git-hooks/` into `.git/hooks/`, marking each executable. Idempotent — re-run whenever a hook is updated.

## Why these aren't in `bin/`

`bin/` holds **user-facing executables** — the `hecks_playground` gem CLI, helpers for building gems, generating examples, running behaviours. Things a user installs and runs.

`tooling/` holds **dev-process scripts** — instruments the framework uses on itself. Things a contributor runs while working on `hecks_playground`. The separation isn't enforced by anything ; it's a courtesy to readers who want to understand what kind of file each one is at a glance.

## See also

- inbox `i118` — the framework reshape arc that put this directory at the top level
- inbox `i124` — the Round 1 story that lifted these specific files (successor to lost `i127`)
