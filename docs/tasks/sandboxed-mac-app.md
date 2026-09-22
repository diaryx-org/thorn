---
title: Thorn on the App Store, through Xcode Cloud
description: The app is signed and sandboxed in Release and a post-clone script builds it on Xcode Cloud; the App Store Connect side — the app record and the workflow — is what is left
author: adammharris
status: in-progress
created: 2026-09-21
updated: 2026-09-22
part_of: '[Tasks](tasks.md)'
---
# Thorn on the App Store, through Xcode Cloud

**Where.** `apps/thorn-editor/project.yml`, `apps/thorn-editor/ci_scripts/`,
and App Store Connect.

**What.** Thorn is a document app — `DocumentGroup`, a declared document
type, an icon, a marketing version — distributed through the App Store and
built by Xcode Cloud, as the Diaryx app is. That settles what the Mac build
needs: the sandbox, signing with the team's certificates, and nothing of
Developer ID, notarisation or a `.dmg`.

What the repository does now:

- **Release is the App Store's shape.** Signed through automatic signing for
  team `V4322HH5HU`, which project.yml carries (a team set in Xcode's Signing
  pane is lost at the next `xcodegen generate`), and sandboxed on the Mac
  through Xcode's entitlement build settings: `ENABLE_APP_SANDBOX` and
  `ENABLE_USER_SELECTED_FILES = readwrite`. A drawing of the profile is one
  file — its style, markers and ink are inside the `<svg>` — so the grant for
  the file the user opened is the whole of it; an `<image href>` to a file
  beside the drawing is the one thing the sandbox would refuse, and the
  profile does not offer one. Debug stays unsigned and unsandboxed on the
  Mac, so `cargo xtask swift` builds on a machine with no certificate.
- **Apple Silicon only on the Mac** (`ARCHS[sdk=macosx*]: arm64`): an archive
  otherwise builds x86_64 too, against a staticlib that has none of it.
- **`ci_scripts/ci_post_clone.sh`** installs XcodeGen, the Rust toolchain
  rust-toolchain.toml names with the three Apple targets, and Zig 0.16.0
  (twig-sys compiles from source for iOS), writes `CI_BUILD_NUMBER` as the
  build number, and generates the project. Package pins come from the
  committed root `Package.resolved`, which xcodebuild reads for the local
  package even with automatic resolution off.
- `ITSAppUsesNonExemptEncryption` is `NO`, and an iPad takes every
  orientation, which App Store validation requires.

Verified on 2026-09-22: a local Release build is signed for the team and
carries exactly the sandbox and user-selected read-write entitlements; run
sandboxed, it opened a drawing, took a new shape, and saved it to disk. The
post-clone script, run in a fresh clone under an empty `HOME`, installed
everything and generated the project, and from it an unsigned
`generic/platform=macOS` archive (arm64, build 42) and a
`generic/platform=iOS` archive both built with only what the script
installed.

**What is left** is App Store Connect's, and Adam's:

- The app record for bundle id `org.diaryx.thorn` (which registers the App ID
  if automatic signing has not).
- An Xcode Cloud workflow on `diaryx-org/thorn`, project
  `apps/thorn-editor/Thorn.xcodeproj`, scheme `Thorn`: an Archive action for
  macOS (and one for iOS, if the phone app ships too) with TestFlight or App
  Store distribution. Cloud-managed signing does the rest.

**Done when.** An Xcode Cloud build of the Mac app reaches TestFlight, and
that build, installed, opens a drawing from the Finder, edits it and saves.
