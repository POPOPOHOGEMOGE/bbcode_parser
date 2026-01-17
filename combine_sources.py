#!/usr/bin/env python3
# combine_sources.py

from pathlib import Path
from itertools import chain

OUTPUT = Path("combined.txt")
if OUTPUT.exists():
    OUTPUT.unlink()

print("[+] Collecting .gd / .tscn files...")

rs_files   = [f for f in Path(".").rglob("*.rs") if "target" not in f.parts]
pest_files = [f for f in Path(".").rglob("*.pest") if "addons" not in f.parts]
cargo_toml = [f for f in Path(".").rglob("Cargo.toml") if "addons" not in f.parts]

# newline="" で **変換を無効化** → CRLF が二重にならない
with OUTPUT.open("w", encoding="utf-8", newline="") as out:
    for file in chain(rs_files, pest_files, cargo_toml):
        out.write(f"{'//'*10}\r\n")
        out.write(f"# {file.resolve()}\r\n")
        out.write(f"{'//'*10}\r\n")
        out.write(file.read_text(encoding="utf-8", errors="ignore"))
        out.write("\r\n\r\n")     # 空行 2 行を CRLF で明示

print(f"[+] Done! Output written to {OUTPUT}")