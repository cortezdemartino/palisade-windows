# Building

Use Node.js 22.13 or newer on Windows x64 and install the locked development dependencies with npm ci.

## Pinned update manager

The v0.3.4 candidate uses a native helper for routine installation. Its manual repair option opens the companion Palisade Installer & Update Manager. The Manager executable is an external build dependency, with its source release, filename, byte length and SHA-256 pinned in assets/manager-release.json. The dependency remains bundled during installed-upgrade qualification.

Download the exact manager asset from the pinned GitHub release into .manager, or set PALISADE_MANAGER_EXE to that verified executable outside the source tree. The packaging hook rejects missing, truncated or substituted manager executables. No sibling checkout is required. Both Actions workflows retrieve and verify this dependency before packaging.

The verified manager is published as the manager-v1.0.1 prerelease build component. Stable desktop update checks ignore that component release. Read-only builds can retrieve the pinned public dependency without accessing draft releases. Replacing the dependency requires a reviewed pin change; never silently replace an existing public release asset. The release job verifies the dependency again before publication.

## Verification and packaging

Run npm run check, npm run test:window-controls, and npm audit --omit=dev --audit-level=high. Build the installer with npm run package:win; output stays in out. Set PALISADE_WINDOW_SMOKE_ASAR to the absolute path of out/win-unpacked/resources/app.asar and run npm run test:window-controls again.

Run `npm run test:update-native` and `npm run test:update-policy` on Windows.
After packaging, run `npm run verify:update-package` to confirm the helper
integrity and its read-only version and parent checks. For the end-to-end
installed test, see [the qualification procedure](docs/U1-INSTALLED-UPGRADE.md).

The updater and Assist helper hashes are recorded before ASAR packaging. The
builder leaves those two executable files untouched during optional
Authenticode signing of the main app and installer.

The Windows UI test temporarily applies the release file-protocol restriction to the development Electron runtime and restores its original setting afterward. Close development Electron processes first. The test verifies each icon's visible pixels, panel sections and actions, hover colors, maximize/restore/minimize/close behavior, and remote-page isolation. Disposable test profiles and screenshots stay under out. The release workflow requires source and archive checks before signing.

## Release

Review docs/RELEASE-0.2.3.md and docs/UPDATE-SIGNING.md. The package version must match its version tag. The protected release environment retains its reviewer and tag restrictions. A signing key is required; missing credentials stop the pipeline. The read-only build/sign job uploads explicit artifacts, and a separate publish job verifies signatures, installer hashes and the manager pin before publication. An existing public release is never overwritten. A staged draft must target the exact tagged commit before it can be published.
