#!/usr/bin/env bash
# Installs proto and the tools .prototools pins (moon, Node, pnpm) in a CI
# job, after the job restored them from the Actions cache (the step "Cache
# the proto toolchain" in .github/workflows/*.yml). The cache key hashes
# .prototools and this file, which pins proto's own version below.
#
# - Warm cache: proto is in ~/.proto/bin at the pinned version and `proto
#   install` finds every tool installed, so nothing is downloaded.
# - Cold cache: proto is downloaded from its GitHub release and checked
#   against the SHA-256 pinned below, then `proto install` downloads the
#   tools. Each download is tried ATTEMPTS times, 10 s and then 20 s apart;
#   a lasting failure fails the job with an error that names the host it
#   could not reach.
#
# scripts/toolchain-workflow.test.mjs runs this against fake curl, sleep and
# proto.
set -euo pipefail

PROTO_VERSION="0.63.2"
# The release archive for this runner, proto_cli-<target>.tar.xz, and its
# SHA-256 as the release's own .sha256 file gives it. A downloaded archive
# that does not match is never unpacked.
release_for() {
	case "$1" in
	"Linux x86_64") target="x86_64-unknown-linux-gnu"; sha256="24df51fc98848abfcc27507a53c2ce005b4da783d4bf662717ae9a9d17432c9a" ;;
	"Linux aarch64") target="aarch64-unknown-linux-gnu"; sha256="5e56d307ab648cfe243d5e9bf968e7e5a1c736e92dac9c6c5925326c64f5ba50" ;;
	"Darwin x86_64") target="x86_64-apple-darwin"; sha256="6d071b05657237f4e3c2e60cee3777564a1b9350eaadf69529e5c9bfa5686867" ;;
	"Darwin arm64") target="aarch64-apple-darwin"; sha256="d2bb0da3afd121baf516b2e8ed7f549b2849243db91bca0a57b398392fba5e41" ;;
	*) return 1 ;;
	esac
}
ATTEMPTS=3
# proto deletes and re-downloads a cached plugin older than its cache
# duration, and a restored cache keeps the files' original times. Ten years
# keeps a warm cache offline; the plugin versions are fixed by proto's own.
CACHE_DURATION=315360000

: "${GITHUB_PATH:?runs in a GitHub Actions job}"
: "${GITHUB_ENV:?runs in a GitHub Actions job}"
PROTO_HOME="${PROTO_HOME:-$HOME/.proto}"
LOG="$(mktemp)"

cd "$(dirname "$0")/.."

export PATH="$PROTO_HOME/shims:$PROTO_HOME/bin:$PATH"
export PROTO_CACHE_DURATION="$CACHE_DURATION"
echo "$PROTO_HOME/bin" >>"$GITHUB_PATH"
echo "$PROTO_HOME/shims" >>"$GITHUB_PATH"
echo "PROTO_CACHE_DURATION=$CACHE_DURATION" >>"$GITHUB_ENV"

# retry WHAT COMMAND...: runs COMMAND up to ATTEMPTS times, waiting longer
# after each failure. Its output is shown and kept in $LOG.
retry() {
	local what="$1" attempt delay
	shift
	for ((attempt = 1; ; attempt++)); do
		if "$@" 2>&1 | tee "$LOG"; then
			return 0
		fi
		if ((attempt >= ATTEMPTS)); then
			return 1
		fi
		delay=$((10 * attempt))
		echo "$what failed (attempt $attempt of $ATTEMPTS), retrying in ${delay}s"
		sleep "$delay"
	done
}

# The host of URL.
host_of() {
	sed -E 's#^[a-z]+://([^/]+).*#\1#' <<<"$1"
}

# The hosts of the URLs in FILE, comma-separated.
hosts_in() {
	{ grep -oE 'https?://[^/ "]+' "$1" || true; } |
		sed -E 's#^https?://##' | sort -u | paste -s -d, -
}

# The SHA-256 of FILE, in hex.
sha256_of() {
	if command -v sha256sum >/dev/null; then
		sha256sum "$1"
	else
		shasum -a 256 "$1"
	fi | awk '{ print $1 }'
}

installed_version() {
	{ "$PROTO_HOME/bin/proto" --version 2>/dev/null || true; } | awk '{ print $2 }'
}

if [[ "$(installed_version)" != "$PROTO_VERSION" ]]; then
	if ! release_for "$(uname -sm)"; then
		echo "::error title=Toolchain install failed::proto has no release for $(uname -sm)"
		exit 1
	fi
	url="https://github.com/moonrepo/proto/releases/download/v$PROTO_VERSION/proto_cli-$target.tar.xz"
	archive="$(mktemp -d)"
	if ! retry "Downloading proto $PROTO_VERSION" \
		curl --fail --location --silent --show-error --output "$archive/proto.tar.xz" "$url"; then
		echo "::error title=Toolchain download failed::proto $PROTO_VERSION could not be downloaded from $(host_of "$url") after $ATTEMPTS attempts: $(tail -n 1 "$LOG")"
		exit 1
	fi
	actual="$(sha256_of "$archive/proto.tar.xz")"
	if [[ "$actual" != "$sha256" ]]; then
		echo "::error title=Toolchain download failed::proto_cli-$target.tar.xz from $(host_of "$url") has SHA-256 $actual, not the pinned $sha256; it was not unpacked"
		exit 1
	fi
	mkdir -p "$archive/proto" "$PROTO_HOME/bin"
	tar -xJf "$archive/proto.tar.xz" -C "$archive/proto" --strip-components=1
	cp "$archive/proto/proto" "$archive/proto/proto-shim" "$PROTO_HOME/bin/"
	chmod 755 "$PROTO_HOME/bin/proto" "$PROTO_HOME/bin/proto-shim"
	if [[ "$(installed_version)" != "$PROTO_VERSION" ]]; then
		echo "::error title=Toolchain install failed::the downloaded proto reports '$(installed_version)', not $PROTO_VERSION"
		exit 1
	fi
fi

# install_or_fail WHAT COMMAND...: retries COMMAND, then fails the job
# naming the hosts in its output.
install_or_fail() {
	local what="$1"
	if ! retry "$@"; then
		hosts="$(hosts_in "$LOG")"
		echo "::error title=Toolchain install failed::$what could not download from ${hosts:-a host it did not name (see its log above)} after $ATTEMPTS attempts"
		exit 1
	fi
}

# The tools .prototools pins.
install_or_fail "proto install" proto install
# moon runs its tasks through the proto in ~/.proto/tools/proto at the
# version .moon/toolchains.yml pins, this one, and would otherwise download
# it on its first task, without retries. Checked first, as moon checks it:
# offline, `proto install proto <version>` fails to resolve the version
# even when it is installed.
if [[ ! -x "$PROTO_HOME/tools/proto/$PROTO_VERSION/proto" ]]; then
	install_or_fail "proto install proto" proto install proto "$PROTO_VERSION"
fi
