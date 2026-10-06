#!/usr/bin/env python3
"""Reuse upstream's checksum-pinned V8 download for the container Cargo build."""
import os
from pathlib import Path
import platform
import sys

root = Path(__file__).resolve().parents[2]
os.environ["CODEX_REPO_ROOT"] = str(root)
sys.path.insert(0, str(root / "scripts"))
from codex_package.targets import TARGET_SPECS
from codex_package.v8 import fetch_codex_v8_artifacts

machine = platform.machine()
target = {"x86_64": "x86_64-unknown-linux-gnu", "aarch64": "aarch64-unknown-linux-gnu"}[machine]
pair = fetch_codex_v8_artifacts(TARGET_SPECS[target], cache_root=root / ".build/v8")
for name, path in (("RUSTY_V8_ARCHIVE", pair.archive), ("RUSTY_V8_SRC_BINDING_PATH", pair.binding)):
    print(f"{name}=/source/{path.relative_to(root)}")
