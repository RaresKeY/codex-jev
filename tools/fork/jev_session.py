"""Own one foreground fork server and publish private discovery metadata."""

import json
import os
from pathlib import Path
import signal
import socket
import stat
import subprocess
import tempfile
import time


def registry():
    base = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"))
    if (
        not base.is_dir()
        or base.stat().st_uid != os.getuid()
        or base.stat().st_mode & 0o077
    ):
        raise RuntimeError("A user-owned XDG_RUNTIME_DIR is required for Jev sessions")
    root = base / "codex-jev" / "sessions"
    root.mkdir(parents=True, mode=0o700, exist_ok=True)
    if root.is_symlink() or root.stat().st_uid != os.getuid():
        raise RuntimeError("Invalid Jev session registry ownership")
    root.chmod(0o700)
    return root


def process_start(pid):
    return Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19]


def server_overrides(args):
    result = []
    flags = iter(args[: args.index("--")] if "--" in args else args)
    for arg in flags:
        if arg in ("-c", "--config", "--enable", "--disable"):
            try:
                result.extend((arg, next(flags)))
            except StopIteration:
                raise ValueError(f"Missing value for {arg}") from None
        elif arg.startswith(("--config=", "--enable=", "--disable=")):
            result.append(arg)
    return result


def run_session(native, args):
    flags = args[: args.index("--")] if "--" in args else args
    if any(arg == "--remote" or arg.startswith("--remote=") for arg in flags):
        raise ValueError("--no-daemon cannot be combined with --remote")
    directory = Path(tempfile.mkdtemp(prefix="s-", dir=registry()))
    endpoint = directory / "server.sock"
    manifest = directory / "session.json"
    if len(os.fsencode(endpoint)) >= 104:
        directory.rmdir()
        raise RuntimeError("Jev session socket path is too long")
    server = client = None
    previous = {}
    try:
        with (directory / "server.log").open("wb") as log:
            server = subprocess.Popen(
                [
                    str(native),
                    *server_overrides(args),
                    "app-server",
                    "--listen",
                    f"unix://{endpoint}",
                ],
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=log,
                start_new_session=True,
                umask=0o077,
            )
        deadline = time.monotonic() + 30
        while True:
            if server.poll() is not None:
                raise RuntimeError(
                    f"Jev foreground server could not start (exit {server.returncode})"
                )
            if endpoint.exists():
                mode = endpoint.stat()
                if not stat.S_ISSOCK(mode.st_mode) or mode.st_uid != os.getuid():
                    raise RuntimeError("Invalid foreground server socket")
                with socket.socket(socket.AF_UNIX) as probe:
                    probe.settimeout(0.2)
                    try:
                        probe.connect(str(endpoint))
                        break
                    except OSError:
                        pass
            if time.monotonic() >= deadline:
                raise TimeoutError("Jev foreground server startup timed out")
            time.sleep(0.05)
        # The terminal sends Ctrl+C to the TUI. The supervisor remains alive so
        # it can reap both processes and remove its exact private directory.
        previous[signal.SIGINT] = signal.signal(signal.SIGINT, lambda *_: None)
        boundary = args.index("--") if "--" in args else len(args)
        client_args = [arg for arg in args[:boundary] if arg != "--no-daemon"] + args[
            boundary:
        ]
        client = subprocess.Popen(
            [str(native), "--remote", f"unix://{endpoint}", *client_args]
        )

        def terminate(signum, _frame):
            if client.poll() is None:
                client.send_signal(signum)

        for signum in (signal.SIGTERM, signal.SIGHUP):
            previous[signum] = signal.signal(signum, terminate)
        data = {
            "schema": 1,
            "program": "codex-jev",
            "ownerPid": os.getpid(),
            "ownerStart": process_start(os.getpid()),
            "serverPid": server.pid,
            "serverStart": process_start(server.pid),
            "clientPid": client.pid,
            "clientStart": process_start(client.pid),
            "socket": str(endpoint),
            "cwd": os.getcwd(),
        }
        temporary = directory / "session.tmp"
        temporary.write_text(json.dumps(data) + "\n")
        temporary.chmod(0o600)
        temporary.replace(manifest)
        code = client.wait()
        return code if code >= 0 else 128 - code
    finally:
        manifest.unlink(missing_ok=True)
        if client is not None and client.poll() is None:
            client.terminate()
            try:
                client.wait(timeout=5)
            except subprocess.TimeoutExpired:
                client.kill()
                client.wait()
        if server is not None:
            try:
                os.killpg(server.pid, signal.SIGTERM)
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(server.pid, signal.SIGKILL)
                server.wait()
            except ProcessLookupError:
                server.wait()
        for signum, handler in previous.items():
            signal.signal(signum, handler)
        for child in directory.iterdir():
            if child.is_file() or child.is_socket():
                child.unlink(missing_ok=True)
        directory.rmdir()
