#!/usr/bin/env python3
"""Checks index.json against each module's manifest and pinned wasm.

Run from the repo root: python3 scripts/check-catalogue.py
"""
import hashlib
import json
import pathlib
import sys

root = pathlib.Path(__file__).resolve().parent.parent
problems = []


def fail(module, message):
    problems.append(f"{module}: {message}")


index = json.loads((root / "index.json").read_text())
listed = {m["id"]: m for m in index["modules"]}
on_disk = {p.parent.name for p in (root / "modules").glob("*/manifest.json") if p.parent.name != "_template"}
for missing in sorted(on_disk - listed.keys()):
    fail(missing, "has a manifest but no index.json row")
for ghost in sorted(listed.keys() - on_disk):
    fail(ghost, "is in index.json but has no manifest")

for mid in sorted(on_disk & listed.keys()):
    m = json.loads((root / "modules" / mid / "manifest.json").read_text())
    row = listed[mid]
    for key in ("name", "version", "summary"):
        if m.get(key) != row.get(key):
            fail(mid, f"index.json {key} differs from the manifest")
    if m["id"] != mid:
        fail(mid, "manifest id does not match its directory")
    wasm = root / m["artifact"]["path"]
    if not wasm.is_file():
        fail(mid, f"artifact {m['artifact']['path']} is missing")
    elif hashlib.sha256(wasm.read_bytes()).hexdigest() != m["artifact"]["sha256"]:
        fail(mid, "artifact sha256 does not match the pinned value")
    keys = {p["key"] for p in m.get("permissions", [])}
    names = {e["name"] for e in m["extension_points"] if e["kind"] == "command"}
    for e in m["extension_points"]:
        if e.get("permission") not in keys:
            fail(mid, f"extension point {e['name']} uses an undeclared permission")
        if e["kind"] in ("slash-command", "code-block-runner", "app") and e.get("command") not in names:
            fail(mid, f"extension point {e['name']} runs an undeclared command")

for p in problems:
    print(p, file=sys.stderr)
sys.exit(1 if problems else 0)
