#!/usr/bin/env python3
"""Shared, process-safe weekly accounting for Jev classifier calls."""

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import sqlite3
import sys
import time
import uuid


DEFAULT_LIMIT = 10_000_000
WEEK_SECONDS = 7 * 24 * 60 * 60
PENDING_SECONDS = 300
MAX_TOKENS = 10**15


class BudgetError(RuntimeError):
    pass


class BudgetExceeded(BudgetError):
    pass


def default_path():
    override = os.environ.get("CODEX_JEV_BUDGET_FILE")
    if override:
        return Path(override).expanduser()
    state_home = os.environ.get("XDG_STATE_HOME")
    root = Path(state_home).expanduser() if state_home else Path.home() / ".local/state"
    return root / "codex-jev/budget.sqlite3"


def token_count(value):
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= MAX_TOKENS:
        raise BudgetError(f"Token count must be an integer from 0 to {MAX_TOKENS}.")
    return value


def timestamp(seconds):
    return datetime.fromtimestamp(seconds, timezone.utc).isoformat().replace("+00:00", "Z")


class Budget:
    def __init__(self, path=None, clock=time.time):
        self.path = Path(path) if path is not None else default_path()
        self.clock = clock

    @contextmanager
    def transaction(self):
        connection = None
        try:
            self.path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            connection = sqlite3.connect(self.path, timeout=5, isolation_level=None)
            os.chmod(self.path, 0o600)
            connection.row_factory = sqlite3.Row
            connection.execute("BEGIN IMMEDIATE")
            connection.execute(
                "CREATE TABLE IF NOT EXISTS settings "
                "(id INTEGER PRIMARY KEY CHECK (id = 1), enabled INTEGER NOT NULL, "
                "limit_tokens INTEGER NOT NULL, window_start INTEGER NOT NULL)"
            )
            connection.execute(
                "CREATE TABLE IF NOT EXISTS calls "
                "(id TEXT PRIMARY KEY, tokens INTEGER NOT NULL, state TEXT NOT NULL, "
                "started_at INTEGER NOT NULL)"
            )
            now = int(self.clock())
            connection.execute(
                "INSERT OR IGNORE INTO settings VALUES (1, 1, ?, ?)", (DEFAULT_LIMIT, now)
            )
            connection.execute(
                "UPDATE calls SET state = 'unknown' WHERE state = 'pending' AND started_at <= ?",
                (now - PENDING_SECONDS,),
            )
            start = connection.execute("SELECT window_start FROM settings").fetchone()[0]
            if now >= start + WEEK_SECONDS:
                connection.execute("DELETE FROM calls WHERE state != 'pending'")
                connection.execute(
                    "UPDATE settings SET window_start = ?",
                    (start + ((now - start) // WEEK_SECONDS) * WEEK_SECONDS,),
                )
            yield connection
            connection.commit()
        except (OSError, sqlite3.Error) as error:
            raise BudgetError("Cannot access Jev budget state; routing stopped.") from error
        finally:
            if connection is not None:
                connection.close()

    def snapshot(self, connection=None):
        if connection is None:
            with self.transaction() as connection:
                return self.snapshot(connection)
        settings = connection.execute("SELECT * FROM settings").fetchone()
        totals = {row[0]: row[1] for row in connection.execute(
            "SELECT state, SUM(tokens) FROM calls GROUP BY state"
        )}
        used = totals.get("settled", 0)
        reserved = totals.get("pending", 0)
        unknown = totals.get("unknown", 0)
        total = used + reserved + unknown
        return {
            "enabled": bool(settings["enabled"]),
            "limit_tokens": settings["limit_tokens"],
            "used_tokens": used,
            "reserved_tokens": reserved,
            "unreported_tokens": unknown,
            "accounted_tokens": total,
            "remaining_tokens": max(0, settings["limit_tokens"] - total),
            "window_start": timestamp(settings["window_start"]),
            "resets_at": timestamp(settings["window_start"] + WEEK_SECONDS),
        }

    def reserve(self, tokens):
        token_count(tokens)
        with self.transaction() as connection:
            state = self.snapshot(connection)
            if state["enabled"] and state["accounted_tokens"] + tokens > state["limit_tokens"]:
                raise BudgetExceeded(
                    "Jev weekly token budget cannot admit this call "
                    f"({state['accounted_tokens']:,}/{state['limit_tokens']:,} tokens accounted; "
                    f"{tokens:,} estimated for this request). "
                    "Use codex-jev budget status, set, reset, or off."
                )
            reservation = uuid.uuid4().hex
            connection.execute("INSERT INTO calls VALUES (?, ?, 'pending', ?)",
                               (reservation, tokens, int(self.clock())))
            return reservation

    def settle(self, reservation, actual_tokens):
        if actual_tokens is not None:
            token_count(actual_tokens)
        with self.transaction() as connection:
            if actual_tokens is None:
                connection.execute(
                    "UPDATE calls SET state = 'unknown' WHERE id = ?", (reservation,)
                )
            else:
                connection.execute(
                    "INSERT INTO calls VALUES (?, ?, 'settled', ?) "
                    "ON CONFLICT(id) DO UPDATE SET tokens = excluded.tokens, state = 'settled'",
                    (reservation, actual_tokens, int(self.clock())),
                )

    def configure(self, command, limit=None):
        with self.transaction() as connection:
            if command == "set":
                connection.execute("UPDATE settings SET limit_tokens = ?", (token_count(limit),))
            elif command in ("on", "off"):
                connection.execute("UPDATE settings SET enabled = ?", (command == "on",))
            elif command == "reset":
                connection.execute("DELETE FROM calls WHERE state != 'pending'")
                connection.execute("UPDATE settings SET window_start = ?", (int(self.clock()),))
            elif command != "status":
                raise BudgetError("Unknown Jev budget command.")
            return self.snapshot(connection)


def main(argv=None):
    arguments = list(sys.argv[1:] if argv is None else argv)
    as_json = "--json" in arguments
    arguments = [value for value in arguments if value != "--json"]
    parser = argparse.ArgumentParser(
        description=__doc__,
        epilog=(
            "Counts Jev input + output tokens across sessions. Calls reserve estimates; "
            "reported usage replaces them. This admission limit is not a provider hard cap. "
            "Pending calls older than five minutes become unreported estimates. Reset and "
            "weekly rollover clear reported/unreported usage and retain recent reservations."
        ),
    )
    parser.add_argument("--json", action="store_true", help="print machine-readable status")
    commands = parser.add_subparsers(dest="command")
    for name in ("status", "reset", "on", "off"):
        commands.add_parser(name)
    commands.add_parser("set").add_argument("tokens", type=int)
    args = parser.parse_args(arguments)
    try:
        state = Budget().configure(args.command or "status", getattr(args, "tokens", None))
    except BudgetError as error:
        if as_json:
            print(json.dumps({"error": str(error)}))
        else:
            print(str(error), file=sys.stderr)
        return 2
    if as_json:
        print(json.dumps(state, separators=(",", ":")))
    else:
        enabled = "on" if state["enabled"] else "off (accounting continues)"
        print(f"Jev weekly budget: {enabled}")
        print(f"Accounted: {state['accounted_tokens']:,} / {state['limit_tokens']:,} tokens")
        print(f"Reported: {state['used_tokens']:,}; reserved: {state['reserved_tokens']:,}; "
              f"unreported estimate: {state['unreported_tokens']:,}")
        print(f"Resets: {state['resets_at']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
