#!/usr/bin/env python3
"""Fails a module whose src has no #[test], so `cargo test` cannot pass on zero tests.

Run from the repo root: python3 scripts/check-module-tests.py

scripts/module-tests-unwritten.txt lists the modules that had no tests when this
check was added. It may only shrink: a listed module that now has a test must be
removed, and a name that is not a module is refused.
"""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
allow_file = "scripts/module-tests-unwritten.txt"
test_attr = re.compile(r"^\s*#\[test\]", re.MULTILINE)
problems = []

lines = ((line.split("#")[0].strip()) for line in (root / allow_file).read_text().splitlines())
allow = {line for line in lines if line}
modules = sorted(p.parent.name for p in (root / "modules").glob("*/Cargo.toml"))

for ghost in sorted(allow - set(modules)):
    problems.append(f"{ghost}: is in {allow_file} but is not a module")

for mid in modules:
    count = sum(len(test_attr.findall(f.read_text())) for f in (root / "modules" / mid / "src").rglob("*.rs"))
    if count == 0 and mid not in allow:
        problems.append(f"{mid}: has no #[test] in src, so `cargo test` passes on nothing; add unit tests")
    if count > 0 and mid in allow:
        problems.append(f"{mid}: now has tests, so remove it from {allow_file}")

for p in problems:
    print(p, file=sys.stderr)
sys.exit(1 if problems else 0)
