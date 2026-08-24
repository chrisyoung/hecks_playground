#!/bin/bash
# statusline-command.sh — thin wrapper for Claude Code's statusline
# harness. Bluebook-of-record : hecks_playground_conception/capabilities/statusline/
# Logic lives in rust/src/run_statusline/ (Rust runner).
#
# Claude Code's settings.json points the statusline action at this
# script (~/.claude/statusline-command.sh is a symlink here). The
# script's only job is to resolve the binary location and exec the
# `storehouse statusline` subcommand ; the runner does the heki reads,
# the consciousness branch, the time animations, and the rendering.
#
# [antibody-exempt: hecks_playground_conception/statusline-command.sh — three-line
#  transitional wrapper for Claude Code's harness ; the contract still
#  expects a .sh file. Retires when ~/.claude/settings.json points at
#  `storehouse statusline` directly without the .sh shim. The 273 lines
#  of rendering logic that used to live here moved to the Rust runner
#  (i97 → i145).]

script="$0"
while [ -L "$script" ]; do script="$(readlink "$script")"; done
script_dir="$(cd "$(dirname "$script")" && pwd)"
hecks_playground_root="$(cd "$script_dir/.." && pwd)"
# REPOINTED (hecksagain cutover, 2026-08-12): the old Rust storehouse
# binary can no longer safely parse hecks_playground_conception, since its content
# is now hecksagain-shaped. hecksagain-cli's own `statusline` subcommand
# is a verified, faithful Ruby port of the same rust/src/run_statusline/
# rendering logic (confirmed byte-identical live output, modulo the
# heart-glyph animation frame) — see hecksagain_runtime/lib/
# hecksagain_runtime/statusline*.rb.
hecks_playground="${STOREHOUSE:-$hecks_playground_root/hecksagain_runtime/bin/hecksagain-cli}"

exec "$hecks_playground" statusline
