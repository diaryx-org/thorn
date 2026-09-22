#!/bin/bash
#
# Xcode Cloud post-clone hook: install what the Thorn app's build needs, then
# generate `apps/thorn-editor/Thorn.xcodeproj` — gitignored, generated from
# project.yml — before Xcode Cloud's first `xcodebuild` call.
#
# The same shape as diaryx's, for the same two reasons: this is the only hook
# that runs before Xcode Cloud resolves packages against the project, and it
# has to live beside the .xcodeproj rather than at the repository root, which
# Xcode Cloud does not search for a nested project.
#
# Package pins need no step here. The app's one package is this repository,
# by local path, and xcodebuild reads the committed root Package.resolved for
# it — including with automatic resolution disabled, as Xcode Cloud runs it.
#
# Nothing is archived, signed or uploaded here; the workflow's Archive action
# does that with cloud-managed signing for team V4322HH5HU, from project.yml.
set -euo pipefail

ROOT="${CI_PRIMARY_REPOSITORY_PATH:?ci_post_clone.sh must run under Xcode Cloud}"
ZIG_VERSION="0.16.0"   # twig's minimum_zig_version

echo "▸ Repository:   $ROOT"
echo "▸ Platform:     ${CI_PRODUCT_PLATFORM:-unknown}"
echo "▸ Build number: ${CI_BUILD_NUMBER:-unset}"

echo "▸ Installing XcodeGen…"
export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1
brew install xcodegen

# Rust. The image ships none, and the project's pre-build phase builds the
# thorn-svg-ffi staticlib with cargo from $HOME/.cargo/bin. The toolchain is
# the one rust-toolchain.toml names, installed from inside the checkout.
echo "▸ Installing Rust…"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --default-toolchain none --profile minimal --no-modify-path
export PATH="$HOME/.cargo/bin:$PATH"
cd "$ROOT"
rustup toolchain install
rustc --version

# Every target now, rather than in the pre-build phase: which one a build
# needs depends on the workflow — darwin for a Mac archive, ios for an iPhone
# one, ios-sim for a Test action.
echo "▸ Adding Rust targets…"
rustup target add aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim

# Zig. twig-sys ships a prebuilt library for macOS but not for iOS, and
# compiles twig's Zig core from source there. Installed under $HOME/.local/bin,
# which the pre-build phase puts on PATH.
echo "▸ Installing Zig ${ZIG_VERSION}…"
ZIG_PREFIX="$HOME/.local/zig-$ZIG_VERSION"
if [ ! -x "$ZIG_PREFIX/zig" ]; then
  ZIG_URL="$(curl -fsSL https://ziglang.org/download/index.json | /usr/bin/python3 -c "
import json, sys
release = json.load(sys.stdin)['$ZIG_VERSION']
for key in ('aarch64-macos', 'macos-aarch64'):
    if key in release:
        print(release[key]['tarball'])
        break
else:
    sys.exit('no aarch64 macOS tarball for Zig $ZIG_VERSION')
")"
  mkdir -p "$ZIG_PREFIX"
  curl -fsSL "$ZIG_URL" | tar -xJ --strip-components=1 -C "$ZIG_PREFIX"
fi
mkdir -p "$HOME/.local/bin"
ln -sf "$ZIG_PREFIX/zig" "$HOME/.local/bin/zig"
export PATH="$HOME/.local/bin:$PATH"
zig version

# CFBundleVersion. project.yml carries 1 for local builds; App Store Connect
# refuses a second upload under one number, so a cloud build takes Xcode
# Cloud's own counter, which the iOS and macOS workflows share. The clone is
# thrown away after the build, so writing the number into project.yml here
# changes nothing anyone commits.
BUILD_NUMBER="${CI_BUILD_NUMBER:-1}"
echo "▸ CFBundleVersion: $BUILD_NUMBER"
sed -i '' "s/^\( *CURRENT_PROJECT_VERSION: \)\"1\"$/\1\"$BUILD_NUMBER\"/" apps/thorn-editor/project.yml
grep -q "CURRENT_PROJECT_VERSION: \"$BUILD_NUMBER\"" apps/thorn-editor/project.yml \
  || { echo "project.yml's CURRENT_PROJECT_VERSION line did not take the build number" >&2; exit 1; }

# The binding is committed, so this is only the project, not bootstrap.sh.
echo "▸ Generating the Xcode project…"
cd "$ROOT/apps/thorn-editor"
xcodegen generate

echo "✓ Post-clone setup complete."
