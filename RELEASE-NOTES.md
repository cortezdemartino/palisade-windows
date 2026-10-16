# Release notes

## 0.2.1 (unreleased)

- Added signed SHA-256 update manifests while retaining the v0.2.0 update format.
- Added download progress, release notes, native update notifications and menu status.
- Added persisted tray preferences, optional launch on login and an About dialog.
- Added a verified installer handoff after shutdown, failure receipts and retry handling.
- Hardened release verification, credential checks and publishing permissions.
- Added Windows upgrade and installer-helper tests.

## 0.2.0

- Added automatic update checks using public GitHub Releases.
- Added Ed25519 release manifest verification and SHA-512 installer integrity checks.
- Added an installation prompt and a manual update check in the application and tray menus.
- Kept the existing Palisade application identity and desktop profile.
- Added locked dependency installation, Windows build checks and a tag-based release workflow.

This release is a manual upgrade for existing 0.1.0 installations. Subsequent versions can use the updater. Windows installers are not Authenticode-signed and have not been independently security audited.

## 0.1.0

Initial Windows desktop client with a restricted Electron window, secure navigation defaults, an independent browser profile and tray support.
