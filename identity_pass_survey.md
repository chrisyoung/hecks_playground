# Identity pass — 63 aggregates missing `identified_by`

Mark up the **proposed key** column (or drop a note) and hand it back — I'll wire whatever you land on. `pick` = an existing attribute looks like the natural key already. `mint` = nothing on the aggregate is unique enough; needs a synthetic/singleton key (I've suggested a shape, following the `Kitchen`/`KitchenName`-style singleton precedent already in the corpus).

## adapters/ (7)

| file | aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|---|
| heki.bluebook | Binding | aggregate_name, store_path, id_strategy, save_strategy | pick | `aggregate_name` | one binding per persisted aggregate |
| heki.bluebook | Reference | from_aggregate, to_aggregate, resolution | mint | composite `from_aggregate`+`to_aggregate` | neither field alone is unique; needs a minted pair-key VO |
| ollama.bluebook | Connection | url, model, status | pick | `url` | one connection per server |
| ollama.bluebook | Inference | prompt, response, model | mint | synthetic id | each call is a one-off; prompts repeat |
| r2.bluebook | Binding | aggregate_name, store_path, id_strategy, save_strategy | pick | `aggregate_name` | same shape as heki's Binding |
| r2.bluebook | Reference | from_aggregate, to_aggregate, resolution | mint | composite `from_aggregate`+`to_aggregate` | same shape as heki's Reference |
| storehouse_api.bluebook | Binding | base_url, bearer, route_path, timeout_seconds | pick | `base_url` | one binding per remote worker |

## catalog/applications/ (4)

| file | aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|---|
| applications.bluebook | Application | name, domain_path, app_types, version, viable_at, status | pick | `domain_path` | folder path is the true unique handle; `name` could collide |
| applications.bluebook | DomainLifecycle | domain_name, stage, nursery_path, catalog_path, application_path | pick | `domain_name` | one lifecycle record per domain |
| applications.bluebook | AppRegistry | total_applications, total_web, total_cli, total_api | mint | singleton, e.g. `identified_by :name` defaulting to `"registry"` | pure counters, no candidate field, exactly one instance ever |
| rvdc/rvdc.bluebook | App | name, status, domains | pick | `name` | also plausibly a singleton — flag if so |

## catalog/bluebook/appeal.bluebook — the IDE domain (34)

| aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|
| Project | name, path, status | pick | `path` | directory path is the real unique handle |
| Explorer | domain_name, aggregate_names, last_opened_path, last_opened_domain (ref Project) | pick | `domain_name` | one explorer view per open domain |
| Document | filename, content, dirty, diagnostics (ref Project) | pick* | `filename` | *only unique within a Project — flag if cross-project collisions matter |
| Timeline | entries, cursor (ref Document) | mint | tie to the Document it belongs to | 1:1 companion, no own field |
| Generator | target, status, output_path, artifacts (ref Project) | pick | `target` | one generator per target language |
| Pattern | name, category, template, parameters | pick | `name` | |
| Feature | title, description, status, additions, counts | pick | `title` | |
| Backlog | name, features | pick | `name` | (or mint if truly one backlog ever) |
| ProductExecutor | active_agent, conversations | mint | singleton | one product-executor session |
| FeatureFlag | feature_title, enabled, permanent | pick | `feature_title` | |
| Review | title, status, author, changes_summary, comments | pick | `title` | |
| Annotation | element_type, element_name, body, resolved, replies | mint | synthetic id | many annotations can target the same element |
| Agent | adapter_mode, thinking, messages, suggestions, loaded_domain | mint | singleton | one AI assistant instance |
| Insight | findings, score | mint | singleton or per-run id | |
| Migration | from_version, to_version, status, changes | mint | synthetic id (or `to_version` if only one live migration kept) | |
| Comparison | left_version, right_version, differences | mint | synthetic id | |
| Diagram | view_type, nodes, edges | pick | `view_type` | one diagram per view type |
| EventStorm | name, status, stickies | pick | `name` | |
| Glossary | terms | mint | singleton | one glossary per project |
| Console | selected_aggregate, selected_command, status, last_result, event_log, form | mint | singleton | one console UI state |
| AcceptanceTest | status, results, passed, failed, total | mint | singleton or per-run id | |
| EventStream | status, events, filter_aggregate, filter_event_type | mint | singleton | one live feed |
| Scenario | name, status, steps, result | pick | `name` | |
| Fixture | name, entries | pick | `name` | |
| Playground | events, state_snapshot | mint | singleton | |
| Workbench | active_project, active_aggregate | mint | singleton | |
| Session (IDE) | mode, connection_status | mint | singleton | |
| Layout | panels, active_tab, sidebar_collapsed, events_panel_collapsed | mint | singleton | |
| LayoutState | current_file, current_domain | mint | singleton | |
| Menu | open_menu, items | mint | singleton | |
| KeyboardShortcut | key, modifiers, command_aggregate, command_name | pick | `key` | the keybinding itself is the natural id |
| Notification | severity, message, context, dismissed | mint | synthetic id | many transient notifications |
| Screenshot | buffer_size, capture_interval, status | mint | singleton | rolling-buffer recorder config |
| Search | query_text, results, result_count | mint | singleton (current search state) | repeated searches would collide on query_text |

## catalog/bluebook/boot.bluebook (2)

| aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|
| Session (boot) | engine, status, started_at, organ_count, nerve_count, law_count, brain_source | pick* | `started_at` | *"one brain, one body, one boot" — could also be a plain singleton |
| BrainProbe | claude_available, ollama_running, summer_adapter_present, recommended_engine | mint | singleton | one probe result |

## catalog/bluebook/catalog.bluebook (4)

| aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|
| Entry | name, version, path, aggregate_count, command_count, policy_count, lines, status | pick | `name` | one entry per domain |
| Version | domain_name, version, snapshot, developed_from, developed_at, aggregate_count, command_count, vector | mint | composite `domain_name`+`version` | many versions per domain; neither field alone is unique |
| Mirror | cannon_path, last_synced_at, domains_synced | mint | singleton | one mirror process |
| Index | catalog_count, nursery_count, total_aggregates, total_commands | mint | singleton | |

## catalog/bluebook/court.bluebook (3)

| aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|
| Case | action_type, actor, target, detail, occurred_at, status, laws_applicable/passed/violated, ruling | mint | synthetic id | many cases over time, no unique field |
| Finding | case_id, law_name, check_expression, evidence, passed, explanation | mint | composite `case_id`+`law_name` | one finding per law checked per case |
| Precedent | law_name, action_type, ruling, reasoning, established_at | mint | composite `law_name`+`action_type` | |

## catalog/bluebook/law.bluebook (3)

| aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|
| Law | name, description, domain_scope, check_expression, severity, status, violation_count | pick | `name` | |
| Violation | law_name, violator, location, detail, detected_at, resolution | mint | synthetic id | many violations of the same law |
| Enforcement | scope, laws_checked/passed/violated, enforced_at, verdict | mint | synthetic id | many runs over time |

## catalog/bluebook/pizzas.bluebook (2) — catalog's own copy of the example

| aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|
| Pizza | name, description, toppings | pick | `name` | |
| Order | customer_name, items, status (ref Pizza) | mint | synthetic id | many orders per customer |

## storehouse/ (4)

| file | aggregate | attributes | tag | proposed key | why |
|---|---|---|---|---|---|
| command_bus.bluebook | CommandBus | app, storage_backend, host, auth_scheme | pick | `app` | the app segment IS the bus's identity — matches i776's own framing when it hit this live |
| dispatch.bluebook | Dispatch | service_name | pick | `service_name` | field already exists for exactly this; default it to a fixed value ("dispatch") since it's a singleton |
| lexicon.bluebook | Lexicon | service_name | pick | `service_name` | same shape, default "lexicon" |
| query.bluebook | Query | service_name | pick | `service_name` | same shape, default "query" |

---

**Totals:** 63 aggregates · ~29 clean `pick` · ~34 `mint` (mostly singleton IDE-surface state or event/log-shaped records with no natural field)
