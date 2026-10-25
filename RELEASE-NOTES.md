# Release notes

## 0.3.0 (unpublished)

- Implement the canonical description-only Assist v1 bridge and pin the actual web adapter in contract tests.
- Add a local screen/window picker and inactive capture controller; keep capture denied because of the Electron permission ambiguity/legacy bypass finding.
- Add isolated lease, coordinate/key-state, clipboard-envelope and file-stream validation components without native or network execution.
- Add Electron picker/bridge/legacy-denial checks, a full-scope handoff and an endpoint/threat review packet.
- Keep capture permission denied independently of controller availability; a feature flag alone cannot enable modern or legacy capture.
- Add a 30-case Windows capture-admission fixture and record the official-runtime comparison, implementation order and session access-pass design defaults.
- Bind offline leases to the original session deadline, check liveness before consent, and cancel revoked capability work without restoring permissions.
- Refuse file-source access before authorization and interrupt stalled receives when cancellation or destination failure occurs.

Native Assist execution, actual sharing, cross-client Assist E2EE and clipboard/files are not qualified or enabled. Publication of the earlier desktop candidate was stopped at the owner's request. The current development target is full Assist release testing; see [the release notes](docs/RELEASE-0.3.0.md) for availability and verification limits.


## 0.2.12 (unpublished preparation)

- Audit the committed web Assist contract dependency; no desktop handoff is present.
- Restrict session permissions to the current trusted main frame and audio-only media checks.
- Explicitly deny display capture without source enumeration or new privileged IPC.
- Add real Electron and packaged permission-boundary checks and integration guidance.

Assist capture and cross-client E2EE acceptance remain blocked or Not Verified. No release or production activation is approved by this candidate.

## 0.2.2 (2026-10-17)

- Integrated the supplied Ed25519 public key for the authorized manual test-client bootstrap.
- Added signed installer URLs and authenticated, version-bound key transitions.
- Kept compatibility manifests and fail-closed release signing checks.
- Added rotation, signature, installer-integrity and key-recovery tests and documentation.

All earlier manual installations have been uninstalled, as confirmed by the operator. Install 0.2.2 manually to establish the current update trust key. Future releases can be authenticated by this installation; earlier old-key executables cannot authenticate this bootstrap.

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
