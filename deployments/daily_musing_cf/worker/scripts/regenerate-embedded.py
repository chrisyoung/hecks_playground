#!/usr/bin/env python3
"""Regenerate worker/src/embedded.rs from ../domain/*.

Embeds every .bluebook and .hecksagon under deployments/daily_musing_cf/domain/
as string constants the Worker parses at request time. `.fixtures` files are
NEVER embedded (fixtures→policies, 2026-07-26) : demo records establish via
`on "BootCompleted"` policies when the generated lib.rs dispatches
CompleteBoot — see boot.bluebook.

The embedded.rs header has referenced this script since i103 ; it now exists.
Run from anywhere : python3 worker/scripts/regenerate-embedded.py
"""
import pathlib

HERE = pathlib.Path(__file__).resolve()
DOMAIN = HERE.parents[2] / "domain"
OUT = HERE.parents[1] / "src" / "embedded.rs"
PRIMARY = "daily_musing.bluebook"

files = sorted(
    [p for p in DOMAIN.iterdir() if p.suffix in (".bluebook", ".hecksagon")],
    key=lambda p: (p.name != PRIMARY, p.name),
)

lines = [
    "// AUTO-GENERATED — daily-musing bluebooks embedded into the Worker WASM.",
    "// To regenerate :",
    "//   python3 worker/scripts/regenerate-embedded.py",
    "//",
    f"// Embedded files : {len(files)}",
    f"// Primary        : {PRIMARY}",
    "// `.fixtures` files are never embedded (fixtures→policies, 2026-07-26) —",
    "// demo records establish via `on \"BootCompleted\"` policies at CompleteBoot.",
    "",
    f'pub const PRIMARY_BASENAME: &str = "{PRIMARY}";',
    f"pub const BLUEBOOK_COUNT:   usize = {len(files)};",
    "",
    "pub static EMBEDDED_BLUEBOOKS: &[(&str, &str)] = &[",
]
for p in files:
    src = p.read_text()
    lines.append(f'    ("{p.name}", r#####"{src}"#####),')
lines.append("];")
OUT.write_text("\n".join(lines) + "\n")
print(f"wrote {OUT} ({len(files)} embedded file(s))")
