#!/usr/bin/env python3
"""Restore and verify the exact img2threejs commit in an external cache."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import sys


HERE = Path(__file__).resolve().parent
PROJECT = "img2threejs"
REVISION = "9fbd0ca5bbcc3b13bebe712745d6784d33db0b85"
SOURCE_URL = "https://github.com/img2threejs/img2threejs.git"
EXPECTED_TREE = "0ee3c2a6d781407808df98b33174539842f85fcc"
EXPECTED_TREE_MANIFEST = "bf4b35eb5b468a77ae6d8a24fc2e4b7a42fa27ad87c2afacd32bf635e069d91e"
EXPECTED_FILE_COUNT = 337
EXPECTED_BLOB_BYTES = 4126550


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"img2threejs snapshot restore failed: {message}")


def run_git(checkout: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(checkout), *args],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        fail(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_tree_digest(checkout: Path) -> tuple[int, int, str]:
    listing = run_git(checkout, "ls-tree", "-r", "--long", REVISION)
    digest = hashlib.sha256()
    file_count = 0
    blob_bytes = 0
    for line in listing.splitlines():
        header, separator, path = line.partition("\t")
        if not separator:
            fail("git ls-tree returned a malformed entry")
        fields = header.split()
        if len(fields) != 4 or fields[1] != "blob":
            fail(f"unexpected tree entry: {line}")
        size = int(fields[3])
        digest.update(f"{path}\t{fields[2]}\t{size}\n".encode("utf-8"))
        file_count += 1
        blob_bytes += size
    return file_count, blob_bytes, digest.hexdigest()


def verify_checkout(checkout: Path) -> None:
    checkout = checkout.resolve()
    if not checkout.is_dir() or not (checkout / ".git").exists():
        fail(f"not a git checkout: {checkout}")
    head = run_git(checkout, "rev-parse", "HEAD").strip()
    if head != REVISION:
        fail(f"HEAD is {head}, expected {REVISION}")
    tree = run_git(checkout, "rev-parse", "HEAD^{tree}").strip()
    if tree != EXPECTED_TREE:
        fail(f"tree is {tree}, expected {EXPECTED_TREE}")
    status = run_git(checkout, "status", "--porcelain")
    if status.strip():
        fail("checkout has uncommitted or untracked files")
    count, blob_bytes, digest = canonical_tree_digest(checkout)
    if (count, blob_bytes, digest) != (EXPECTED_FILE_COUNT, EXPECTED_BLOB_BYTES, EXPECTED_TREE_MANIFEST):
        fail(
            "tree inventory mismatch: "
            f"count={count} bytes={blob_bytes} digest={digest}"
        )
    license_path = checkout / "LICENSE"
    expected_license = "4595055948a67e91177115c57e154804046878e77ff223de22accc880012827a"
    if not license_path.is_file() or sha256_file(license_path) != expected_license:
        fail("upstream LICENSE hash mismatch")
    print(f"img2threejs pinned snapshot verified: {checkout}")


def safe_cache_root(value: Path) -> Path:
    if not value.is_absolute():
        fail("--cache-root must be an absolute, non-root path")
    root = value.resolve()
    if root == Path("/") or root == Path.home():
        fail("--cache-root must be an absolute, non-root path")
    repository_root = HERE.parents[5]
    try:
        root.relative_to(repository_root)
    except ValueError:
        return root
    fail("--cache-root must be outside the Weaponry product tree")


def restore(cache_root: Path) -> None:
    root = safe_cache_root(cache_root)
    target = root / PROJECT / REVISION
    if target.is_symlink():
        fail("refusing to follow a cache checkout symlink")
    if target.exists():
        verify_checkout(target)
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    run = lambda *args: subprocess.run(
        list(args),
        check=True,
        cwd=target,
        capture_output=True,
        text=True,
    )
    target.mkdir()
    run("git", "init", "-q")
    run("git", "remote", "add", "origin", SOURCE_URL)
    run("git", "fetch", "--depth=1", "origin", REVISION)
    run("git", "checkout", "--detach", REVISION)
    verify_checkout(target)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--cache-root", type=Path, help="restore into this external cache root")
    action.add_argument("--verify", type=Path, metavar="CHECKOUT", help="verify an existing checkout")
    args = parser.parse_args()
    if args.verify is not None:
        verify_checkout(args.verify)
    else:
        restore(args.cache_root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
