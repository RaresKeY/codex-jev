"""Bounded live old/new comparison on public synthetic thread fixtures. No retries."""

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
import os
from pathlib import Path
import random
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
CASES = ROOT / "codex-rs/tui/tests/fixtures/jev_context_cases.json"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, choices=(1, 2, 3), default=2)
    parser.add_argument("--extra-cases", type=Path)
    parser.add_argument(
        "--context-mode",
        choices=("old-visible-new-transcript", "both-transcript"),
        default="old-visible-new-transcript",
        help="Use both-transcript to isolate policy changes with identical evidence.",
    )
    args = parser.parse_args()
    if args.output.exists():
        parser.error(
            "Choose a new output file; existing evidence is never overwritten."
        )
    cases = json.loads(CASES.read_text())
    if args.extra_cases:
        cases.extend(json.loads(args.extra_cases.read_text()))
    if len({case["id"] for case in cases}) != len(cases):
        parser.error("Case IDs must be unique.")
    helpers = {"old": args.baseline_dir / "route.py", "new": ROOT / "jev/route.py"}
    env = os.environ.copy()
    pointer = (
        Path(env.get("XDG_CONFIG_HOME", str(Path.home() / ".config")))
        / "codex-jev/key-file"
    )
    if not env.get("CODEX_JEV_KEY_FILE") and pointer.is_file():
        env["CODEX_JEV_KEY_FILE"] = pointer.read_text().strip()
    hashes = {
        label: {
            name: hashlib.sha256((helper.parent / name).read_bytes()).hexdigest()
            for name in ("route.py", "policy.json")
        }
        for label, helper in helpers.items()
    }
    jobs = [
        (case, label, repeat)
        for case in cases
        for label in helpers
        for repeat in range(args.repeats)
    ]
    random.Random(20261028).shuffle(jobs)

    def run(job):
        case, label, repeat = job
        context = dict(case["legacy"])
        if label == "new" or args.context_mode == "both-transcript":
            context["transcript"] = case["transcript"]
        started = time.monotonic()
        row = {
            "case": case["id"],
            "variant": label,
            "repeat": repeat,
            "input_sha256": hashlib.sha256(
                json.dumps(
                    {"task": case["task"], "context": context}, sort_keys=True
                ).encode()
            ).hexdigest(),
        }
        try:
            result = subprocess.run(
                [sys.executable, "-I", str(helpers[label])],
                input=json.dumps({"task": case["task"], "context": context}).encode(),
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                env=env,
                timeout=36,
                check=False,
            )
            value = json.loads(result.stdout)
            if result.returncode or "error" in value:
                row["error"] = "helper_rejected"
            else:
                row.update({key: value[key] for key in ("model", "effort", "stats")})
                row["model_match"] = value["model"] in case["expected_models"]
                row["effort_match"] = value["effort"] in case["expected_efforts"]
                row["pair_match"] = row["model_match"] and row["effort_match"]
        except (subprocess.TimeoutExpired, ValueError, KeyError):
            row["error"] = "invalid_or_timed_out"
        row["seconds"] = round(time.monotonic() - started, 3)
        return row

    rows = []
    with ThreadPoolExecutor(max_workers=2) as pool:
        for future in as_completed([pool.submit(run, job) for job in jobs]):
            row = future.result()
            rows.append(row)
            print(
                f"{len(rows)}/{len(jobs)} {row['variant']} {row['case']}: "
                f"{row.get('model', row.get('error'))} {row.get('effort', '')}",
                flush=True,
            )
    summary = {}
    for label in helpers:
        selected = [row for row in rows if row["variant"] == label]
        summary[label] = {
            "requests": len(selected),
            "errors": sum("error" in row for row in selected),
            **{
                field: sum(row.get(field, False) for row in selected)
                for field in ("model_match", "effort_match", "pair_match")
            },
            "median_seconds": round(
                statistics.median(row["seconds"] for row in selected), 3
            ),
            "input_tokens": sum(
                row.get("stats", {}).get("input_tokens", 0) for row in selected
            ),
            "output_tokens": sum(
                row.get("stats", {}).get("output_tokens", 0) for row in selected
            ),
        }
    evidence = {
        "schema": 1,
        "repeats": args.repeats,
        "context_mode": args.context_mode,
        "hashes": hashes,
        "cases_sha256": hashlib.sha256(CASES.read_bytes()).hexdigest(),
        "extra_cases_sha256": (
            hashlib.sha256(args.extra_cases.read_bytes()).hexdigest()
            if args.extra_cases
            else None
        ),
        "summary": summary,
        "results": sorted(
            rows, key=lambda row: (row["case"], row["repeat"], row["variant"])
        ),
        "limits": "Synthetic predeclared routing rubric; no executor quality or calibration measurement. "
        "Context mode records whether both helpers receive the same transcript or only the candidate does. "
        "At most two concurrent helper processes and no retries.",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
