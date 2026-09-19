#!/usr/bin/env python3
# Validates the scenes a module actually emits against slim-m's renderer.
#
# The renderer drops an op it cannot parse and falls back on a colour name it
# does not know, both silently. So a module can build, pass its unit tests,
# run correctly under wasmi, and still draw a board with no numbers on it -
# which is exactly how minesweeper 0.1.0 shipped. Nothing in the toolchain
# compared the emitted JSON against the client that has to read it.
#
# This does. Point it at a built module and the frames you want checked:
#
#   scripts/check-scene-ops.py modules/minesweeper/0.1.0/module.wasm play ""
#
# It is deliberately a copy of the contract rather than an import of it: the
# renderer lives in the slim-m repo, and a registry that could only be
# validated from a checkout of another repo would not get validated.
import json
import subprocess
import sys

# client/packages/app/lib/src/widgets/module_scene_painter.dart, resolveSceneColor.
COLOURS = {
    "bg", "surface", "sunken", "accent", "accent-soft",
    "muted", "text", "border", "danger",
}

# client/packages/app/lib/src/widgets/module_scene.dart, _parseOp. The value is
# the set of keys that op actually reads; anything else is ignored, and a key
# the op needs but does not get makes the whole op vanish.
OPS = {
    "cells": {"cols", "rows", "data", "palette", "gap", "tap", "tap_batch"},
    "rect": {"x", "y", "w", "h", "fill", "stroke", "sw", "r", "tap"},
    "circle": {"cx", "cy", "r", "fill", "stroke", "sw", "tap"},
    "line": {"x1", "y1", "x2", "y2", "stroke", "sw"},
    "text": {"x", "y", "s", "fill", "size", "align"},
    "path": {"d", "fill", "stroke", "sw", "tap"},
    "input": {"submit", "x", "y", "w", "value", "placeholder", "max"},
    "notes": {"notes"},
}

# An op that is dropped entirely when this key is missing.
REQUIRED = {"cells": "data", "text": "s", "notes": "notes", "path": "d", "input": "submit"}

# Keys that name a colour, so a typo falls back rather than failing.
COLOUR_KEYS = {"fill", "stroke"}


def run_module(runner, wasm, command, payload):
    out = subprocess.run(
        [runner, wasm, command, payload], capture_output=True, text=True
    )
    if out.returncode != 0:
        raise SystemExit(f"module run failed: {out.stderr.strip()}")
    answer = json.loads(out.stdout)
    if not answer.get("ok"):
        raise SystemExit(f"module answered an error: {answer.get('error')}")
    return answer["output"]


def check_scene(raw):
    """Every problem with one emitted scene, as a list of sentences."""
    problems = []
    try:
        scene = json.loads(raw)
    except json.JSONDecodeError:
        return ["output is not JSON, so it renders as plain text"]
    if scene.get("$slim") != "scene/1":
        return ["output is not tagged scene/1, so it renders as plain text"]

    for name in ("background",):
        value = scene.get(name)
        if value and not value.startswith("#") and value not in COLOURS:
            problems.append(f"{name}: unknown colour {value!r}")

    for index, op in enumerate(scene.get("ops", [])):
        kind = op.get("op")
        where = f"ops[{index}] ({kind})"
        if kind not in OPS:
            problems.append(f"{where}: unknown op, the renderer drops it")
            continue
        need = REQUIRED.get(kind)
        if need and need not in op:
            problems.append(
                f"{where}: no {need!r} key, so the renderer drops this op entirely"
            )
        for key, value in op.items():
            if key == "op":
                continue
            if key not in OPS[kind]:
                problems.append(f"{where}: key {key!r} is not read by this op")
            if key in COLOUR_KEYS and isinstance(value, str):
                if not value.startswith("#") and value not in COLOURS:
                    problems.append(
                        f"{where}: {key} {value!r} is not a scene colour; "
                        "it silently falls back"
                    )
    return problems


def main(argv):
    if len(argv) < 4:
        print(
            "usage: check-scene-ops.py <runner> <module.wasm> <command> [input ...]",
            file=sys.stderr,
        )
        return 2
    runner, wasm, command = argv[1], argv[2], argv[3]
    inputs = argv[4:] or [""]

    failed = 0
    for payload in inputs:
        label = payload[:40] or "<launch>"
        problems = check_scene(run_module(runner, wasm, command, payload))
        if problems:
            failed += 1
            print(f"FAIL {label}")
            for problem in problems:
                print(f"  - {problem}")
        else:
            print(f"ok   {label}")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
