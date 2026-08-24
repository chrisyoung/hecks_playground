---
name: hecks_playground-feature-docs
description: 'Feature documentation workflow for the HecksPlayground framework. Use after completing a feature to update FEATURES.md, create usage docs, and demonstrate the feature with real running output.'
license: MIT
metadata:
  author: hecks_playground
  version: "1.0.0"
---

# HecksPlayground Feature Documentation

After every feature is completed, follow this workflow to document it properly.

## Step 1: Update FEATURES.md

`FEATURES.md` is the canonical feature list for the entire framework.

- Read the current file first to understand the section structure
- Add new entries under the appropriate section heading
- Use the same bullet-point style as existing entries
- Be specific: name the DSL method, class, or CLI command
- One bullet per capability, not per implementation detail

### Section Guide

| Section | What goes here |
|---------|---------------|
| Domain Modeling DSL | New DSL keywords, block syntax, attribute types |
| Extensions | New extension gems or extension registry features |
| Runtime API | New methods on `HecksPlayground`, `Runtime`, or domain modules |
| Code Generation | New generated artifacts, spec types, or build features |
| Persistence | Repository, adapter, or query features |
| CLI Commands | New or changed `hecks_playground` subcommands |
| Rails Integration | ActiveHecks, generators, HecksLive features |
| AI-Native | MCP tools, llms.txt, structured errors |
| Static Domain Generation | hecks_playground_static Ruby or Go target features |

## Step 2: Create Usage Doc

Create `docs/usage/<feature>.md` with:

1. A one-line description of what the feature does
2. A runnable code example using the Pizzas or Banking domain
3. Expected output (copy from actual execution)
4. Any configuration or prerequisites

### Template

```markdown
# Feature Name

Description of what this feature does and when to use it.

## Usage

\```ruby
# Runnable example
HecksPlayground.domain "Example" do
  # ...
end
\```

## Output

\```
# Paste actual output here
\```
```

## Step 3: Show Real Running Examples

This is mandatory. After documenting, run the example and show the user real output.

- Use `ruby -Ilib` to run examples
- Paste actual terminal output, not paraphrased summaries
- If the feature is interactive (REPL, web UI), describe the steps and show what the user will see
- For generated code, show a snippet of the generated output

## What NOT to Document

- Internal refactors that don't change user-facing behavior
- Bug fixes (unless they restore a previously documented feature)
- Test-only changes
