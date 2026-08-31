#!/bin/bash
#
# SPDX-FileCopyrightText: 2026 Vincent Vanackere <vincent.vanackere@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build the static Linux "poexam" binary against musl libc:
#   1. build the release binary for the host musl target
#   2. check that it is statically linked and runs
#   3. optionally write the release archive and its checksum sidecar
#
# The musl targets enable "crt-static" by default, so the binary has no libc
# dependency and runs on any Linux distribution, including Alpine and scratch
# containers.
#
# The build is native, not a cross-build: it only targets the host
# architecture, so run it on x86_64 for x86_64 and on aarch64 for aarch64.
#
# Usage: tools/build-musl.sh [--target=<arch>] [--package] [--out=<dir>]
#
#   --target=<arch>  architecture to build, asserted against the host: x86_64,
#                    aarch64, or either full musl triple (default: the host)
#   --package        also write dist/poexam-linux-<arch>.tar.gz and its
#                    dist/poexam-linux-<arch>.sha256 sidecar
#   --out=<dir>      directory for the outputs (default: dist)

set -euo pipefail

# Move to the repository root (this script lives in tools/).
cd "$(dirname "$0")/.."

# Fixed timestamp for the archive members, so the archive does not depend on
# the date it was built: 1980-01-01T00:00:00Z.
epoch=315532800

req_arch=""
package=""
out=dist
for arg in "$@"; do
    case "${arg}" in
        # Reject empty values instead of falling back to the default.
        --target=|--out=)
            echo "error: ${arg%=} needs a value" >&2
            exit 2
            ;;
        --target=*) req_arch=${arg#*=} ;;
        --package) package=1 ;;
        --out=*) out=${arg#*=} ;;
        *)
            echo "error: unknown argument: ${arg} (--target=<arch>, --package, --out=<dir>)" >&2
            exit 2
            ;;
    esac
done

case "$(uname -m)" in
    x86_64|amd64) host_arch=x86_64 ;;
    aarch64|arm64) host_arch=aarch64 ;;
    *)
        echo "error: no musl release target for $(uname -m)" >&2
        exit 2
        ;;
esac

arch=${host_arch}
if [ -n "${req_arch}" ]; then
    case "${req_arch}" in
        x86_64|amd64|x86_64-unknown-linux-musl) arch=x86_64 ;;
        aarch64|arm64|aarch64-unknown-linux-musl) arch=aarch64 ;;
        *)
            echo "error: --target must be x86_64 or aarch64 (or either musl triple), not '${req_arch}'" >&2
            exit 2
            ;;
    esac
    if [ "${arch}" != "${host_arch}" ]; then
        echo "error: --target=${req_arch} on a ${host_arch} host: the musl build is native, run it on a ${arch} machine" >&2
        exit 2
    fi
fi
target=${arch}-unknown-linux-musl
platform=linux-${arch}

# Check the tools needed to check and package the binary before building it.
tools="file"
if [ -n "${package}" ]; then
    tools="${tools} tar gzip sha256sum"
fi
for tool in ${tools}; do
    command -v "${tool}" >/dev/null || { echo "error: ${tool} is required" >&2; exit 1; }
done

rustup target add "${target}"
cargo build --release --target "${target}"

mkdir -p "${out}"
# Replace the binary atomically: a rename never fails with "Text file busy" if
# the previous binary is still running.
cp "target/${target}/release/poexam" "${out}/.poexam.tmp"
mv -f "${out}/.poexam.tmp" "${out}/poexam"
binary=${out}/poexam

# A dynamically linked binary would defeat the purpose of the musl build, so
# fail rather than ship it ("static-pie linked" or "statically linked").
description=$(file -b "${binary}")
case "${description}" in
    *static*) ;;
    *)
        echo "error: ${binary} is not statically linked: ${description}" >&2
        exit 1
        ;;
esac

# Smoke test: the binary must run on this host and report the built version.
version=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
reported=$("${binary}" --version)
case "${reported}" in
    *"${version}"*) ;;
    *)
        echo "error: ${binary} --version reports '${reported}', expected version ${version}" >&2
        exit 1
        ;;
esac

echo "Built ${binary}: ${description}"

[ -n "${package}" ] || exit 0

# Package the binary, the README and the license, all at the root of the
# archive so a single file can be extracted with:
#   tar -xzf poexam-linux-<arch>.tar.gz poexam
#
# Member order, ownership, modes and timestamps are fixed, and gzip -n omits
# the name and modification time it would otherwise store, so the same binary
# always packages to the same bytes.
archive=poexam-${platform}.tar.gz
sidecar=poexam-${platform}.sha256
stage=$(mktemp -d)
trap 'rm -rf "${stage}"' EXIT
install -m 0755 "${binary}" "${stage}/poexam"
install -m 0644 README.md LICENSE "${stage}/"
# The mode and timestamp of the staging directory itself are not stored: only
# the members listed below are added to the archive.
touch -h -d "@${epoch}" "${stage}"/*

out_abs=$(cd "${out}" && pwd)
# Publish the archive and its sidecar only once both are complete.
tar --format=gnu --owner=0 --group=0 --numeric-owner --mtime="@${epoch}" \
    -cf - -C "${stage}" LICENSE README.md poexam \
    | gzip -9n > "${out_abs}/.${archive}.tmp"
# The sidecar names the bare archive, so "sha256sum -c" works from its
# directory.
digest=$(sha256sum "${out_abs}/.${archive}.tmp" | cut -d' ' -f1)
printf '%s  %s\n' "${digest}" "${archive}" > "${out_abs}/.${sidecar}.tmp"
mv -f "${out_abs}/.${archive}.tmp" "${out_abs}/${archive}"
mv -f "${out_abs}/.${sidecar}.tmp" "${out_abs}/${sidecar}"

echo "Wrote ${out}/${archive}"
cat "${out_abs}/${sidecar}"
