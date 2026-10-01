#!/usr/bin/env bash
# Render the Homebrew formula for a release.
#
#   packaging/homebrew/render.sh VERSION DIST_DIR > lazyaiden.rb
#
# VERSION is the version without the leading "v" (e.g. 0.2.0 or 0.2.0-rc.1).
# DIST_DIR holds the release archives' "<archive>.sha256" files.
# A prerelease version renders the opt-in `lazyaiden-rc` formula instead of `lazyaiden`.
set -euo pipefail

version=${1:?usage: render.sh VERSION DIST_DIR}
dist=${2:?usage: render.sh VERSION DIST_DIR}
repo=${GITHUB_REPOSITORY:-mohammadalijf/lazyaiden}

if [[ $version == *-* ]]; then
  class=LazyaidenRc conflicts=lazyaiden
else
  class=Lazyaiden conflicts=lazyaiden-rc
fi

sha() {
  local file="$dist/lazyaiden-v$version-$1.tar.gz.sha256"
  [[ -f $file ]] || { echo "missing $file" >&2; exit 1; }
  awk '{print $1}' "$file"
}

# Plain assignments so a missing checksum aborts under `set -e`.
sha_mac_arm=$(sha aarch64-apple-darwin)
sha_mac_x86=$(sha x86_64-apple-darwin)
sha_linux_arm=$(sha aarch64-unknown-linux-gnu)
sha_linux_x86=$(sha x86_64-unknown-linux-gnu)

sed \
  -e "s|@CLASS@|$class|g" \
  -e "s|@CONFLICTS@|$conflicts|g" \
  -e "s|@VERSION@|$version|g" \
  -e "s|@BASE_URL@|https://github.com/$repo/releases/download/v$version|g" \
  -e "s|@SHA_AARCH64_APPLE_DARWIN@|$sha_mac_arm|g" \
  -e "s|@SHA_X86_64_APPLE_DARWIN@|$sha_mac_x86|g" \
  -e "s|@SHA_AARCH64_UNKNOWN_LINUX_GNU@|$sha_linux_arm|g" \
  -e "s|@SHA_X86_64_UNKNOWN_LINUX_GNU@|$sha_linux_x86|g" \
  "$(dirname "$0")/lazyaiden.rb.in"
