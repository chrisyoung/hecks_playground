---
name: hecks_playground-skill-maintenance
description: 'Keep HecksPlayground custom skills up to date as the codebase evolves. Use after any change that affects DSL syntax, codebase structure, contracts, conventions, or workflows. Checks each skill for staleness and updates it.'
license: MIT
metadata:
  author: hecks_playground
  version: "1.0.0"
---

# HecksPlayground Skill Maintenance

Custom HecksPlayground skills live in `skills/hecks_playground-*/SKILL.md`. They document the codebase, DSL, contracts, and workflows — and they go stale when the code changes. This skill ensures they stay current.

## When to Check

After any change that touches:

| Change | Skills to check |
|--------|----------------|
| DSL keywords, block syntax, attribute types | `hecks_playground-bluebook-dsl` |
| Directory renames, new modules, moved files | `hecks_playground-navigator` |
| New or changed data contracts | `hecks_playground-data-contracts` |
| Pre-commit steps, CI, or commit conventions | `hecks_playground-precommit` |
| Documentation workflow or FEATURES.md format | `hecks_playground-feature-docs` |
| Large rename or restructure completed | `hecks_playground-rename-playbook` (add to past renames table) |

## How to Update

1. Read the affected `skills/hecks_playground-*/SKILL.md`
2. Compare against the current state of the code
3. Edit the skill to reflect reality
4. Bump the `version` in the frontmatter (patch increment)

## What to Watch For

- **New DSL keywords** not listed in `hecks_playground-bluebook-dsl`
- **Moved directories** that make `hecks_playground-navigator` paths wrong
- **New contracts** missing from `hecks_playground-data-contracts`
- **Changed conventions** (e.g., new commit rules, new pre-commit steps) not in `hecks_playground-precommit`
- **Past renames** not recorded in `hecks_playground-rename-playbook`

## Rules

- Skills describe what IS, not what WAS — remove outdated content, don't comment it out
- Keep skills concise — they're reference material, not tutorials
- Don't add speculative content ("we might add X") — only document what exists
- Test that examples in skills actually work before committing updates
