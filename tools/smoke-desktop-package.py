#!/usr/bin/env python3
"""Launch an extracted desktop archive from a separate working directory and inspect its first frame."""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parent / "walk"))
import bridgeauth  # noqa: F401: authenticates this owned instance's instrument requests
from walk_env import isolated_env


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--out", type=Path, default=Path("target/package-smoke"))
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="octoscode-package-") as temp:
        root = Path(temp)
        shutil.unpack_archive(str(args.archive.resolve()), root / "extracted")
        binary = root / "extracted/OctosCode" / ("octoscode.exe" if os.name == "nt" else "octoscode")
        cwd = root / "unrelated-working-directory"
        cwd.mkdir()
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        env = {k: v for k, v in os.environ.items() if not k.startswith(("OCTOS_", "OCTOSCODE_", "OCTOSENSE_", "MAKEPAD_"))}
        env.update(isolated_env(root / "state"))
        env["MAKEPAD_REMOTE"] = str(port)
        env["OCTOSCODE_DESIGN_DIR"] = str(root / "materialized-design")
        base = f"http://127.0.0.1:{port}"

        def get(path):
            return urllib.request.urlopen(base + path, timeout=5).read()

        with (out / "app.log").open("w", encoding="utf-8") as log:
            proc = subprocess.Popen([str(binary)], cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                snapshot = None
                for _ in range(120):
                    if proc.poll() is not None:
                        raise RuntimeError(f"app exited with {proc.returncode}; see {out / 'app.log'}")
                    try:
                        candidate = json.loads(get("/snap"))
                        widgets = candidate.get("s", [])
                        if any(w.get("i") == "first_run" and w.get("r", [0, 0, 0, 0])[2] > 0 for w in widgets):
                            snapshot = candidate
                            break
                    except (OSError, ValueError):
                        pass
                    time.sleep(0.5)
                if snapshot is None:
                    raise RuntimeError("the packaged app did not render its first-run screen")
                png = get("/g?raw=1")
                if not png.startswith(b"\x89PNG\r\n\x1a\n"):
                    raise RuntimeError("instrument did not return a PNG frame")
                (out / "window.png").write_bytes(png)
                (out / "snapshot.json").write_text(json.dumps(snapshot, indent=2), encoding="utf-8")
                print("PASS: packaged app renders first-run UI and a frame from an unrelated working directory.", flush=True)
            finally:
                try:
                    get("/gq")
                except OSError:
                    pass
                try:
                    proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    proc.terminate()
                    try:
                        proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        proc.kill()
                        proc.wait()
        errors = [line for line in (out / "app.log").read_text(errors="replace").splitlines()
                  if "[E]" in line or "Failed to load resource" in line]
        if errors:
            raise RuntimeError("application/resource errors: " + "\n".join(errors[:10]))


if __name__ == "__main__":
    main()
