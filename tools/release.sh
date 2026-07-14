#!/bin/bash
#
# SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Make a new Poexam release:
#   1. check formatting, run clippy, build, run the tests and the docs
#   2. bump the version
#   3. update the changelog
#   4. commit and tag

set -euo pipefail

# Move to the repository root (this script lives in tools/).
cd "$(dirname "$0")/.."

# Get the version from Cargo.toml and strip any suffix (e.g. 0.0.13-dev -> 0.0.13).
full_version=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
version=${full_version%%-*}
date=$(date +%Y-%m-%d)

echo "Releasing version ${version} (${date})"

# Check that working directory is clean.
if [ -n "$(git status --porcelain)" ]; then
    echo "error: working directory not clean"
    exit 1
fi

# Sanity check: the changelog must have an [Unreleased] section to release.
if ! grep -q '^## \[Unreleased\]$' CHANGELOG.md; then
    echo "error: no [Unreleased] section found in CHANGELOG.md" >&2
    exit 1
fi

# Sanity check: the [Unreleased] link is needed to derive the compare URLs.
if ! grep -q '^\[Unreleased\]: ' CHANGELOG.md; then
    echo "error: no [Unreleased] link found in CHANGELOG.md" >&2
    exit 1
fi

# Check REUSE/licensing compliance before touching any file.
reuse lint

# Check formatting, run clippy (pedantic), build, run the tests and the docs.
# These run before any file is modified, so a failure leaves the working
# directory untouched (nothing to revert, safe to re-run).
cargo fmt --check
cargo clippy -- -D clippy::pedantic
cargo build
cargo test
cargo doc

# Update the version in Cargo.toml (first version line, i.e. the [package] one).
sed -i -E "0,/^version = \".*\"$/s//version = \"${version}\"/" Cargo.toml

# Derive the compare-URL base and the previous tag from the [Unreleased] link.
# e.g. [Unreleased]: https://github.com/poexam/poexam/compare/v0.0.12...HEAD
unreleased_link=$(grep -m1 '^\[Unreleased\]: ' CHANGELOG.md | sed -E 's/^\[Unreleased\]: //')
base_url=${unreleased_link%%/compare/*}
prev_tag=${unreleased_link##*/compare/}
prev_tag=${prev_tag%%...HEAD}

# Update the changelog:
#   - turn the [Unreleased] heading into the released version and date
#   - point the [Unreleased] link at the new version
#   - add the new version link (kept in descending order, after [Unreleased])
sed -i \
    -e "s|^## \[Unreleased\]$|## [${version}] - ${date}|" \
    -e "s|^\[Unreleased\]: .*|[Unreleased]: ${base_url}/compare/v${version}...HEAD|" \
    -e "/^\[Unreleased\]: /a [${version}]: ${base_url}/compare/${prev_tag}...v${version}" \
    CHANGELOG.md

# Rebuild so Cargo.lock records the bumped version before committing.
cargo build

# Commit and tag (only the release files, ignoring anything else staged).
git commit -m "Version ${version}" -- Cargo.toml Cargo.lock CHANGELOG.md
git tag -a "v${version}" -m "Version ${version}"

echo "Version ${version} released and tagged (v${version})."
echo "Push with: git push && git push --tags"
echo "Publish with: cargo publish"
