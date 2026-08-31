#!/bin/bash
#
# SPDX-FileCopyrightText: 2026 Vincent Vanackere <vincent.vanackere@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Print the CHANGELOG.md section of a release, to use as release notes.
#
# Usage: tools/release-notes.sh v0.1.0 [> notes.md]
#
# The section is the "## [<version>] - <date>" heading matching the tag, up to
# the next level-two heading. A missing or empty section is an error.

set -euo pipefail

# Move to the repository root (this script lives in tools/).
cd "$(dirname "$0")/.."

tag=${1:-}
[ -n "${tag}" ] || { echo "error: a release tag is required (vX.Y.Z)" >&2; exit 2; }
[ "$#" -eq 1 ] || { echo "error: expected a single argument, the release tag" >&2; exit 2; }
case "${tag}" in
    v[0-9]*) ;;
    *) echo "error: '${tag}' is not a v-prefixed release tag" >&2; exit 2 ;;
esac

# Print the section body, holding back blank lines so the trailing ones are
# dropped.
notes=$(awk -v heading="## [${tag#v}]" '
    index($0, "## ") == 1 {
        inside = (index($0, heading) == 1)
        next
    }
    !inside { next }
    # Skip the link definitions at the end of the file.
    /^\[[^]]+\]: / { next }
    /^[[:space:]]*$/ { if (seen) { pending = pending "\n" } ; next }
    { printf "%s%s\n", pending, $0; pending = ""; seen = 1 }
' CHANGELOG.md)

if [ -z "${notes}" ]; then
    echo "error: no '## [${tag#v}]' section with a body in CHANGELOG.md" >&2
    exit 1
fi
printf '%s\n' "${notes}"
