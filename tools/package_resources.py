#!/usr/bin/env python3
"""Copy every `resources/` directory of a package's dependency graph into an app bundle.

    cargo metadata --format-version 1 [--filter-platform <host>] | package_resources.py <package> <dest>

<dest> is the bundle's makepad package root (OctosCode.app/Contents/Resources/makepad). Each
crate that has a `resources/` directory beside its Cargo.toml lands under
<dest>/<lib name>/resources/, the path makepad's packaged loader asks for
(platform/src/script/res.rs: `<package_root>/<crate>/<file>` for `crate_resource("<crate>:<file>")`).
Prints one line per crate and the total. Used by tools/package-macos.sh (A33).
"""
import json
import os
import shutil
import sys


def lib_name(pkg):
    for t in pkg.get("targets", []):
        if "lib" in t.get("kind", []) or "rlib" in t.get("kind", []) or "proc-macro" in t.get("kind", []):
            return t["name"].replace("-", "_")
    return pkg["name"].replace("-", "_")


def main():
    root_name, dest = sys.argv[1], sys.argv[2]
    meta = json.load(sys.stdin)
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next((p["id"] for p in meta["packages"] if p["name"] == root_name and p["id"] in nodes), None)
    if root is None:
        sys.exit(f"package_resources: {root_name} is not in the resolved graph")
    # The normal (non-dev, non-build) dependency closure of the app.
    seen, todo = set(), [root]
    while todo:
        pid = todo.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid].get("deps", []):
            kinds = {k.get("kind") for k in dep.get("dep_kinds", [])}
            if kinds & {None, "normal"}:
                todo.append(dep["pkg"])
    total = 0
    copied = 0
    os.makedirs(dest, exist_ok=True)
    for pid in sorted(seen, key=lambda i: pkgs[i]["name"]):
        pkg = pkgs[pid]
        src = os.path.join(os.path.dirname(pkg["manifest_path"]), "resources")
        if not os.path.isdir(src):
            continue
        out = os.path.join(dest, lib_name(pkg), "resources")
        if os.path.exists(out):
            shutil.rmtree(out)
        # Android-only payloads never ride in a desktop bundle (cargo-makepad skips them too).
        shutil.copytree(src, out, ignore=shutil.ignore_patterns("android", ".DS_Store"))
        size = sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(out) for f in fs)
        total += size
        copied += 1
        print(f"package_resources: {lib_name(pkg)}/resources {size / 1e6:.1f} MB")
    print(f"package_resources: {copied} crates, {total / 1e6:.1f} MB of resources in {len(seen)} packages")


if __name__ == "__main__":
    main()
