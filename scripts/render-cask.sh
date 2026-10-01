#!/usr/bin/env bash
#
# Print the Homebrew cask for Thorn.app — `Casks/thorn-editor.rb` in
# diaryx-org/homebrew-tap — for one release:
#
#   scripts/render-cask.sh <version> <sha256 of Thorn-<version>-aarch64.dmg>
#
# mac-app.yml runs this on a release tag and pushes the result to the tap; the
# tap is written by CI and by nothing else. It is a script rather than a
# heredoc in the workflow so that the cask can be rendered and `brew audit`ed
# on a Mac before a tag depends on it.
#
# `thorn-editor`, not `thorn`: `thorn` is the CLI's binary (apps/thorn-svg),
# and the name a formula for it would take — as Leaf's app is `leaf-editor`
# beside the `leaf` formula.
#
# The app is sandboxed, so what it keeps lives in its container, not in
# ~/Library/Preferences or ~/Library/Caches.
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 <version> <sha256>" >&2
  exit 2
fi
version="$1"
sha256="$2"

cat <<RUBY
cask "thorn-editor" do
  version "$version"
  sha256 "$sha256"

  url "https://github.com/diaryx-org/thorn/releases/download/v#{version}/Thorn-#{version}-aarch64.dmg"
  name "Thorn"
  desc "Drawing editor for SVG diagrams"
  homepage "https://github.com/diaryx-org/thorn"

  depends_on arch: :arm64
  depends_on macos: :ventura

  app "Thorn.app"

  zap trash: [
    "~/Library/Application Scripts/org.diaryx.thorn",
    "~/Library/Containers/org.diaryx.thorn",
  ]
end
RUBY
