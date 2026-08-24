Run the project watchers and fix every issue they report.

## Steps

1. Run the watchers:
```bash
ruby -I hecks_playground_watchers/lib -r hecks_playground_watchers -e 'HecksWatchers::PreCommit.new(project_root: Dir.pwd).call'
```

2. Read the output carefully. For each issue reported:

   - **FEATURES.md not updated** — Read the git diff (`git diff --cached` or `git diff`) to understand what changed, then update FEATURES.md with a concise description of the new feature in the appropriate section.

   - **CHANGELOG not updated** — Read the diff for that component, then add a dated entry to that component's CHANGELOG.md.

   - **Files approaching 200-line limit** — Read the flagged file and extract a concern into a new file to bring it under 180 lines.

   - **Cross-component require_relative** — Replace the `require_relative` with a bare `require` for the cross-component dependency.

   - **New files without specs** — Create a spec file for the new class with meaningful tests.

   - **New files missing from autoloads.rb** — Add the autoload entry to `hecksties/lib/hecks_playground/autoloads.rb`.

3. After fixing all issues, run the watchers again to confirm everything is clean.

4. Stage the fixes alongside the original changes.
