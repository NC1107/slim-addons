#!/usr/bin/env python3
"""Launches every app in the catalogue and runs its first frame through check-scene-ops.

Usage: python3 scripts/check-app-scenes.py <run-module-binary>
"""
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
runner = sys.argv[1]
failed = False
for manifest in sorted((root / "modules").glob("*/manifest.json")):
    if manifest.parent.name == "_template":
        continue
    m = json.loads(manifest.read_text())
    for command in sorted({e["command"] for e in m["extension_points"] if e["kind"] == "app"}):
        wasm = root / m["artifact"]["path"]
        done = subprocess.run([sys.executable, str(root / "scripts/check-scene-ops.py"), runner, str(wasm), command, ""])
        print(f"{m['id']} {command}: {'ok' if done.returncode == 0 else 'FAILED'}")
        failed = failed or done.returncode != 0
sys.exit(1 if failed else 0)
