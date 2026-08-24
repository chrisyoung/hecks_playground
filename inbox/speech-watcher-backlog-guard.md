# speech_stream_advance needs a backlog guard — a revived watcher must not speak history

Found 2026-07-27. The watcher had been dead since the Procfile moved to
deploy/ ; its state held last_position=436 against a 3.5 MB transcript.
On revival it started dispatching Voice.Speak for DAYS-old sentences
(“All gates green: parity 388/388…” from a previous session) and the
catch-up blocked its tick for minutes. I fast-forwarded the state to EOF
by hand — that recovery should be structural :

- On tick, if (file_size - last_position) > threshold (say 64 KB), skip
  to EOF, dispatch UpdatePosition, speak nothing. A voice is live
  narration ; history is never narration.
- Also worth checking : what re-points transcript_path at the CURRENT
  session on SessionStart — this morning it still pointed at yesterday's
  transcript, so this session's voice is idle.

Script : hecks_playground_conception/bin/speech_stream_advance (the SpeechStream
bluebook's exec arm — guard belongs in the bluebook's vocabulary too :
a CatchUp/SkipBacklog event, not just python).
