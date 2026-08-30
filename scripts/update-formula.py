#!/usr/bin/env python3
"""Point the Homebrew formula at a released version.

Sets `version` and, for every `url` line in the formula, the `sha256` of the
matching release artifact. Run by the release workflow once the artifacts are
built; also runnable by hand:

    scripts/update-formula.py 0.3.3 artifacts/

Every artifact the formula references must be present in the artifact
directory — a missing one is an error, never a silently kept stale checksum.
"""

import argparse
import hashlib
import pathlib
import re
import sys

URL_RE = re.compile(r'^(\s*)url ".*/(?P<asset>[^/"]+)"\s*$')
# Matches a checksum line in any state we may find one: a real digest, a
# placeholder, or either of those commented out. All are replaced outright, so
# a stale checksum never survives next to a fresh one.
SHA_RE = re.compile(r'^\s*#?\s*sha256 "[^"]*"\s*(#.*)?$')
VERSION_RE = re.compile(r'^(\s*version )"[^"]*"(\s*)$')


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def update(formula: str, version: str, artifact_dir: pathlib.Path) -> tuple[str, int]:
    lines = formula.splitlines(keepends=True)
    out: list[str] = []
    updated = 0
    missing: list[str] = []
    saw_version = False

    i = 0
    while i < len(lines):
        line = lines[i]

        m = VERSION_RE.match(line)
        if m and not saw_version:
            out.append(f'{m.group(1)}"{version}"{m.group(2)}')
            saw_version = True
            i += 1
            continue

        m = URL_RE.match(line)
        if not m:
            out.append(line)
            i += 1
            continue

        # The formula interpolates #{version} into the URL, so resolve it the
        # same way Homebrew would before looking the artifact up on disk.
        asset = m.group("asset").replace("#{version}", version)
        indent = m.group(1)
        out.append(line)
        i += 1

        artifact = artifact_dir / asset
        if not artifact.is_file():
            missing.append(asset)
            # Drop any existing checksum rather than leave a stale one behind.
            if i < len(lines) and SHA_RE.match(lines[i]):
                i += 1
            continue

        digest = sha256(artifact)
        replacement = f'{indent}sha256 "{digest}"\n'
        if i < len(lines) and SHA_RE.match(lines[i]):
            if lines[i] != replacement:
                updated += 1
            i += 1
        else:
            updated += 1
        out.append(replacement)

    if not saw_version:
        sys.exit("error: no `version \"...\"` line found in the formula")
    if missing:
        sys.exit(
            "error: artifacts referenced by the formula are missing from "
            f"{artifact_dir}: {', '.join(missing)}"
        )

    return "".join(out), updated


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help='release version without the "v" prefix')
    parser.add_argument("artifact_dir", type=pathlib.Path, help="directory holding the release artifacts")
    parser.add_argument(
        "--formula",
        type=pathlib.Path,
        default=pathlib.Path("Formula/ghdash.rb"),
        help="formula to rewrite (default: Formula/ghdash.rb)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit non-zero if the formula would change, without writing it",
    )
    args = parser.parse_args()

    if args.version.startswith("v"):
        sys.exit(f'error: pass the version without the "v" prefix, got {args.version!r}')
    if not args.artifact_dir.is_dir():
        sys.exit(f"error: artifact directory not found: {args.artifact_dir}")

    original = args.formula.read_text()
    updated, changed_checksums = update(original, args.version, args.artifact_dir)

    if updated == original:
        print(f"{args.formula} is already up to date for {args.version}")
        return

    if args.check:
        sys.exit(f"error: {args.formula} is out of date for {args.version}")

    args.formula.write_text(updated)
    print(f"{args.formula} updated to {args.version} ({changed_checksums} checksum(s) changed)")


if __name__ == "__main__":
    main()
