---
name: hecks_playground-data-contracts
description: 'Data contract system for HecksPlayground cross-target code generation. Use when working with generators, templates, or ensuring Ruby/Go parity. Covers all 8 contracts: Type, View, Event, EventLog, Migration, Aggregate, Display, FormParsing, UILabel.'
license: MIT
metadata:
  author: hecks_playground
  version: "1.0.0"
---

# HecksPlayground Data Contracts

Data contracts guarantee that Ruby and Go targets produce structurally identical output from the same domain IR. They live in `hecks_playground_templating/` and are the single source of truth for cross-target generation.

## Why Contracts Exist

When generating code for multiple targets (Ruby, Go), the same DSL must produce:
- Identical HTTP API responses
- Identical event shapes
- Identical form parsing behavior
- Identical UI labels and display formatting

Contracts codify these guarantees. Templates consume contract methods — never raw IR fields.

## The 8 Contracts

### 1. TypeContract

Single type registry mapping DSL types to target-specific representations.

| DSL Type | Ruby | Go | SQL | JSON | OpenAPI |
|----------|------|-----|-----|------|---------|
| String | String | string | TEXT | string | string |
| Integer | Integer | int64 | INTEGER | integer | integer |
| Float | Float | float64 | REAL | number | number |
| Boolean | Boolean | bool | BOOLEAN | boolean | boolean |
| Date | Date | string | DATE | string | string (date) |
| DateTime | DateTime | string | DATETIME | string | string (date-time) |
| JSON | Hash | map[string]interface{} | TEXT | object | object |

Key method: `format_go_literal(type, value)` — produces typed comparisons for Go templates.

Location: `hecks_playground_templating/lib/hecks_playground_templating/type_contract.rb`

### 2. ViewContract

Shapes for view data passed to templates (show pages, index tables, forms).

- Short ID display (first 8 chars of UUID)
- Go struct generation from view fields
- Consistent field ordering across targets

Location: `hecks_playground_templating/lib/hecks_playground_templating/view_contract.rb`

### 3. EventContract

Event interface requirements:

- Every event must have `aggregate_id` and `occurred_at` fields
- Event name method: `event_name` (Ruby) / `EventName()` (Go)
- Events are immutable facts — frozen in Ruby, value structs in Go

Location: `hecks_playground_templating/lib/hecks_playground_templating/event_contract.rb`

### 4. EventLogContract

JSON shape for the `/_events` endpoint — identical format in Ruby and Go:

```json
[
  {
    "event": "CreatedPizza",
    "aggregate_id": "abc-123",
    "occurred_at": "2025-01-01T00:00:00Z",
    "data": { "name": "Margherita" }
  }
]
```

Location: `hecks_playground_templating/lib/hecks_playground_templating/event_log_contract.rb`

### 5. MigrationContract

Validates round-trip serialization fidelity for domain diffs used in SQL migration generation.

Location: `hecks_playground_templating/lib/hecks_playground_templating/migration_contract.rb`

### 6. AggregateContract

Standard fields, validations, enums, lifecycle, and self-reference detection.

Key features:
- `standard_fields` — fields every aggregate gets (id, timestamps)
- `agg_suffixes` — suffix-matching for self-referencing ID detection (e.g., `policy_id` matches `GovernancePolicy`)
- Enum constraint enforcement
- Lifecycle default status on create

Location: `hecks_playground_templating/lib/hecks_playground_templating/aggregate_contract.rb`

### 7. DisplayContract

Cell rendering, lifecycle transitions, aggregate summaries, policy labels, home page data.

- How to render each type in table cells
- Lifecycle badge colors and transition hint maps
- Policy label formatting

Location: `hecks_playground_templating/lib/hecks_playground_templating/display_contract.rb`

### 8. FormParsingContract

Type coercion for form submissions:

- Go: generates parse lines (`strconv.ParseInt`, `strconv.ParseFloat`, etc.)
- Ruby: generates coerce expressions (`.to_i`, `.to_f`, etc.)
- Handles list_of fields, references, enums, booleans

Location: `hecks_playground_templating/lib/hecks_playground_templating/form_parsing_contract.rb`

### 9. UILabelContract

PascalCase splitting and pluralization for human-readable labels:

- `GovernancePolicy` → "Governance Policy" (singular) → "Governance Policies" (plural)
- Uses ActiveSupport pluralization rules
- `plural_label` method for nav and index headings

Location: `hecks_playground_templating/lib/hecks_playground_templating/ui_label_contract.rb`

## How to Use Contracts

### In Templates

Templates should always call contract methods rather than computing values inline:

```ruby
# Good — uses contract
TypeContract.go_type(attribute.type)

# Bad — hardcoded mapping
attribute.type == "Integer" ? "int64" : "string"
```

### Adding a New Contract

1. Create `hecks_playground_templating/lib/hecks_playground_templating/<name>_contract.rb`
2. Define the contract as a module with class methods
3. Add specs that verify both Ruby and Go targets satisfy the contract
4. Update templates to use contract methods instead of inline logic

### Validating Contracts

Run the full spec suite — contract specs verify cross-target parity:

```bash
bundle exec rspec
```

Contract violations show up as spec failures with clear messages about which target diverged.

## Key Rule

**Never regex-patch templates.** If Ruby and Go templates need to produce the same output, add a contract method that both targets call. Regex substitution in generated code is fragile and untestable.
