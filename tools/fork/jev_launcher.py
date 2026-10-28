#!/usr/bin/env python3
"""Installed Jev entry point: fork updates and foreground session ownership."""

import os
from pathlib import Path
import sys

from jev_session import run_session
from jev_update import maybe_offer_update, update_command


def interactive_command(args):
    values = {
        "-c",
        "--config",
        "-m",
        "--model",
        "-p",
        "--profile",
        "-C",
        "--cd",
        "-s",
        "--sandbox",
        "-a",
        "--ask-for-approval",
        "--remote",
        "--enable",
        "--disable",
        "--add-dir",
        "-i",
        "--image",
    }
    commands = {
        "exec",
        "e",
        "review",
        "login",
        "logout",
        "mcp",
        "mcp-server",
        "app-server",
        "completion",
        "sandbox",
        "debug",
        "apply",
        "a",
        "cloud",
        "features",
        "remote-control",
        "doctor",
        "help",
    }
    items = iter(args)
    for item in items:
        if item == "--":
            return True
        if item in values:
            next(items, None)
        elif not item.startswith("-"):
            return item not in commands
    return True


def main():
    release = Path(__file__).resolve().parent
    args = sys.argv[1:]
    if args[:1] == ["update"]:
        return update_command(release, args[1:])
    interactive = interactive_command(args)
    flags = args[: args.index("--")] if "--" in args else args
    informational = any(arg in flags for arg in ("--help", "-h", "--version", "-V"))
    if interactive and not informational and sys.stdin.isatty() and sys.stdout.isatty():
        maybe_offer_update(release, args)
    native = release / "codex-jev-native"
    if interactive and not informational and "--no-daemon" in flags:
        return run_session(native, args)
    os.execv(native, [str(native), *args])


if __name__ == "__main__":
    raise SystemExit(main())
