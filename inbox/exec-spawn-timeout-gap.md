# Process.Spawn has no timeout — a hung script bricks its organ loop

Found 2026-07-27 (wake session). speech_stream_advance blocked one loop
tick for 2m24s (stale-backlog catch-up) and the 200ms Advance cadence
simply stopped — no tick, no log, no sentinel. The hexagon chapter's own
principle says an adapter may be SLOW but slow never propagates,
“bounded by the family's timeout” — the payment family declares
`timeout_ms`, but the Process.Spawn primitive (exec_dispatcher) enforces
no bound at all : `wait_with_output` waits forever.

Shape of the fix (bluebook first) : the timeout belongs on the FAMILY /
driving declaration (the hecksagon that binds the exec), and the kernel
leaf gains the enforcement — kill + `error="exec timed out after Nms"`,
ok=false, cascade as any failed spawn. Needs reader threads or polling
around child.wait ; ~30 LoC at the kernel floor.

Related : fix/spawn-cwd-contract (92f25189f) fixed WHERE the child runs ;
this card is HOW LONG it may run.
