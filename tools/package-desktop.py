#!/usr/bin/env python3
"""Build a portable Linux/Windows release with Makepad resources beside the executable."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=Path("target/desktop-packages"))
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    system = {"Linux": "linux", "Windows": "windows"}.get(platform.system())
    if system is None:
        parser.error("use tools/package-macos.sh for macOS")
    arch = {"amd64": "x86_64", "aarch64": "arm64"}.get(
        platform.machine().lower(), platform.machine().lower()
    )
    out = args.out.resolve()
    env = dict(os.environ, MAKEPAD_PACKAGE_DIR="makepad")
    subprocess.run(
        ["cargo", "build", "--locked", "--profile", "app-bundle", "-p",
         "octoscode-desktop", "--bin", "octoscode"], cwd=repo, env=env, check=True
    )
    rustc = subprocess.check_output(["rustc", "-vV"], text=True)
    host = next(line.split(": ", 1)[1] for line in rustc.splitlines() if line.startswith("host: "))
    metadata = subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", host],
        cwd=repo, env=env,
    )
    meta = json.loads(metadata)
    binary = "octoscode.exe" if system == "windows" else "octoscode"
    source = Path(meta["target_directory"]) / "app-bundle" / binary
    if not source.is_file():
        raise SystemExit(f"missing native build: {source}; cross compilation is not supported by this packager")
    bundle = out / "OctosCode"
    if bundle.exists():
        shutil.rmtree(bundle)
    bundle.mkdir(parents=True)
    shutil.copy2(source, bundle / binary)
    subprocess.run(
        [sys.executable, str(repo / "tools/package_resources.py"), "octoscode-desktop", str(bundle / "makepad")],
        input=metadata, check=True,
    )
    shutil.copy2(repo / "LICENSE", bundle / "LICENSE")
    icons = repo / "crates/octoscode-desktop/resources"
    shutil.copy2(icons / "icon_256.png", bundle / "octoscode.png")
    shutil.copy2(icons / "icon.ico", bundle / "octoscode.ico")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    version = next(p["version"] for p in meta["packages"] if p["name"] == "octoscode-desktop")
    info = {"version": version, "commit": commit, "platform": system, "architecture": arch,
            "target": host, "profile": "app-bundle", "rustc": rustc.splitlines()[0]}
    (bundle / "BUILD-INFO.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    archive = Path(shutil.make_archive(
        str(out / f"OctosCode-{system}-{arch}"), "zip" if system == "windows" else "gztar",
        root_dir=out, base_dir="OctosCode",
    ))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
    print(f"Packaged {archive}", flush=True)


if __name__ == "__main__":
    main()
