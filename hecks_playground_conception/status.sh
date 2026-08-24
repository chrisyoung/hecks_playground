#!/bin/sh
# [antibody-exempt: thin shebang-redirector shim from old status.sh; dispatches to capabilities/status/status.bluebook via storehouse run. Retires when the Unix `#!` convention is replaced by a `hecks_playground` CLI multiplexer.]
DIR="$(cd "$(dirname "$0")" && pwd)"
# Resolve storehouse binary : env first, sibling-repo target second,
# bare on PATH last. Without the fallbacks the bare `storehouse` exec
# fails when this script runs in environments that don't have it on
# PATH (smoke-test sandboxes, CI runners, Claude Code subprocess envs).
HECKS_PLAYGROUND="${HECKS_PLAYGROUND_BIN:-}"
[ -z "$HECKS_PLAYGROUND" ] && [ -x "$DIR/../rust/target/release/storehouse" ] && \
  HECKS_PLAYGROUND="$DIR/../rust/target/release/storehouse"
[ -z "$HECKS_PLAYGROUND" ] && HECKS_PLAYGROUND="storehouse"
exec "$HECKS_PLAYGROUND" run "$DIR/../cli/status/status.bluebook" "$@"
