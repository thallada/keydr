#!/usr/bin/env python3
"""Convert raw.githubusercontent.com URLs in code_syntax.rs from branch refs to commit-SHA permalinks.

Usage:
    # Dry-run (prints what would change):
    python3 scripts/permalinkify_code_urls.py --dry-run

    # Apply in-place:
    python3 scripts/permalinkify_code_urls.py

    # With a GitHub token for higher rate limits (recommended for 485 URLs):
    GITHUB_TOKEN=ghp_xxx python3 scripts/permalinkify_code_urls.py

The script resolves each branch ref (master, main, dev, etc.) to the current
commit SHA via the GitHub API, then rewrites the URLs so they never change when
upstream repos push new commits or restructure files.

Before:
    https://raw.githubusercontent.com/tokio-rs/tokio/master/tokio/src/sync/mutex.rs
After:
    https://raw.githubusercontent.com/tokio-rs/tokio/a1b2c3d.../tokio/src/sync/mutex.rs
"""

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

CODE_SYNTAX_PATH = os.path.join(
    os.path.dirname(__file__), "..", "src", "generator", "code_syntax.rs"
)

# Looks like a full 40-char SHA already
SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def github_headers():
    token = os.environ.get("GITHUB_TOKEN")
    headers = {"Accept": "application/vnd.github.v3+json"}
    if token:
        headers["Authorization"] = f"token {token}"
    return headers


def _try_resolve_branch(owner: str, repo: str, ref: str) -> str | None:
    """Try to resolve a single branch name to its commit SHA."""
    url = f"https://api.github.com/repos/{owner}/{repo}/git/ref/heads/{ref}"
    req = urllib.request.Request(url, headers=github_headers())
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read())
            return data["object"]["sha"]
    except urllib.error.HTTPError:
        return None


def _try_resolve_tag(owner: str, repo: str, ref: str) -> str | None:
    """Try to resolve a tag name to its commit SHA."""
    url = f"https://api.github.com/repos/{owner}/{repo}/git/ref/tags/{ref}"
    req = urllib.request.Request(url, headers=github_headers())
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read())
            obj = data["object"]
            if obj["type"] == "tag":
                deref_url = obj["url"]
                req2 = urllib.request.Request(deref_url, headers=github_headers())
                with urllib.request.urlopen(req2, timeout=15) as resp2:
                    tag_data = json.loads(resp2.read())
                    return tag_data["object"]["sha"]
            return obj["sha"]
    except urllib.error.HTTPError:
        return None


def resolve_ref_to_sha(owner: str, repo: str, ref: str) -> str | None:
    """Resolve a branch/tag ref to its commit SHA via the GitHub API.

    Tries the ref as a branch first, then as a tag.  If neither works and the
    ref doesn't contain a slash, also tries common slash-prefixed variants
    (e.g. "master" might actually be the first segment of "master/next").
    """
    if SHA_RE.match(ref):
        return ref

    sha = _try_resolve_branch(owner, repo, ref)
    if sha:
        return sha

    sha = _try_resolve_tag(owner, repo, ref)
    if sha:
        return sha

    print(f"  WARNING: could not resolve {owner}/{repo} ref={ref}", file=sys.stderr)
    return None


def check_rate_limit():
    """Print current GitHub API rate limit status."""
    req = urllib.request.Request(
        "https://api.github.com/rate_limit", headers=github_headers()
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read())
            core = data["resources"]["core"]
            remaining = core["remaining"]
            limit = core["limit"]
            reset_ts = core["reset"]
            reset_in = max(0, reset_ts - int(time.time()))
            print(f"GitHub API rate limit: {remaining}/{limit} remaining, resets in {reset_in}s")
            if remaining < 50:
                print(
                    "WARNING: Low rate limit. Set GITHUB_TOKEN env var for 5000 req/hr.",
                    file=sys.stderr,
                )
            return remaining
    except Exception as e:
        print(f"Could not check rate limit: {e}", file=sys.stderr)
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print changes without modifying the file",
    )
    parser.add_argument(
        "--file",
        default=CODE_SYNTAX_PATH,
        help="Path to code_syntax.rs",
    )
    args = parser.parse_args()

    with open(args.file) as f:
        content = f.read()

    # Collect unique (owner, repo, ref) tuples to minimize API calls.
    # Branch names can contain slashes (e.g. "series/3.x"), so we can't simply
    # split on "/" to extract the ref.  Instead we use the GitHub API to look up
    # the repo's default branch and resolve from there.
    url_prefix_re = re.compile(
        r"https://raw\.githubusercontent\.com/(?P<owner>[^/]+)/(?P<repo>[^/]+)/(?P<rest>.+)"
    )
    urls_found = url_prefix_re.findall(content)

    # Deduce (owner, repo, ref, path) — if `rest` starts with a 40-char hex SHA
    # it's already pinned; otherwise ask the GitHub API for the default branch.
    ref_keys: dict[tuple[str, str, str], str | None] = {}
    for owner, repo, rest in urls_found:
        first_segment = rest.split("/")[0]
        if SHA_RE.match(first_segment):
            ref_keys[(owner, repo, first_segment)] = first_segment
        else:
            # We need to figure out which part of `rest` is the ref vs the path.
            # We try the first segment, then first two segments (for slash-branches
            # like "series/3.x"), etc.
            ref_key = (owner, repo, first_segment)
            if ref_key not in ref_keys:
                ref_keys[ref_key] = None

    already_pinned = sum(1 for _, _, ref in ref_keys if SHA_RE.match(ref))
    to_resolve = sum(1 for _, _, ref in ref_keys if not SHA_RE.match(ref))

    print(f"Found {len(urls_found)} URLs across {len(ref_keys)} unique (owner/repo/ref) combos")
    print(f"  Already pinned to SHA: {already_pinned}")
    print(f"  Need resolution: {to_resolve}")

    if to_resolve == 0:
        print("Nothing to do — all URLs already use commit SHAs.")
        return

    remaining = check_rate_limit()
    if remaining is not None and remaining < to_resolve:
        print(
            f"ERROR: Need {to_resolve} API calls but only {remaining} remaining. "
            "Set GITHUB_TOKEN or wait for reset.",
            file=sys.stderr,
        )
        sys.exit(1)

    # Resolve each unique ref
    resolved = 0
    failed = 0
    for (owner, repo, ref) in sorted(ref_keys):
        if SHA_RE.match(ref):
            ref_keys[(owner, repo, ref)] = ref
            continue

        sha = resolve_ref_to_sha(owner, repo, ref)
        if sha:
            ref_keys[(owner, repo, ref)] = sha
            resolved += 1
            if not args.dry_run:
                # Be polite to the API
                time.sleep(0.1)
        else:
            failed += 1
        # Progress
        done = resolved + failed
        if done % 10 == 0 or done == to_resolve:
            print(f"  Progress: {done}/{to_resolve} ({resolved} resolved, {failed} failed)")

    print(f"\nResolved {resolved}/{to_resolve} refs ({failed} failures)")

    # Build replacement map
    replacements = 0
    new_content = content
    for (owner, repo, ref), sha in ref_keys.items():
        if sha and sha != ref:
            old_prefix = f"raw.githubusercontent.com/{owner}/{repo}/{ref}/"
            new_prefix = f"raw.githubusercontent.com/{owner}/{repo}/{sha}/"
            count = new_content.count(old_prefix)
            if count > 0:
                if args.dry_run:
                    print(f"  {owner}/{repo}: {ref} -> {sha[:12]}... ({count} URLs)")
                new_content = new_content.replace(old_prefix, new_prefix)
                replacements += count

    print(f"\nTotal URL replacements: {replacements}")

    if args.dry_run:
        print("\n(dry-run mode — no file modified)")
    else:
        with open(args.file, "w") as f:
            f.write(new_content)
        print(f"Wrote {args.file}")


if __name__ == "__main__":
    main()
