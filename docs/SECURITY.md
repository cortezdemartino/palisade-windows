# Security model

The Windows client is a desktop wrapper around the official Palisade web application. Message encryption and device verification remain the responsibility of the Matrix SDK and the Palisade service.

## Desktop boundary

The Electron renderer is sandboxed, runs without Node integration or a preload bridge, and can navigate only within `https://palisade.onyxops.org`. External HTTPS links require confirmation before opening in the system browser. Non-HTTPS schemes, camera access, screen capture and privileged device APIs are blocked. Standard TLS validation remains enabled.

The desktop client uses its own persistent browser partition. It does not use Windows DPAPI or the TPM to independently protect Matrix tokens or cryptographic material. Signing out or revoking a session remains essential after device loss.

## Update boundary

- Update checks use the public GitHub Releases API for `cortezdemartino/palisade-windows`.
- Only stable, non-draft releases with a numeric `vMAJOR.MINOR.PATCH` tag are considered.
- The `release.json` asset must contain an Ed25519 signature from the key embedded in this application.
- The signed data fixes the version, installer filename, size, SHA-256 and SHA-512 checksums and channel.
- Downloads use an exact GitHub release path. Redirects are limited to HTTPS GitHub release-asset hosts.
- The installer is downloaded to a dedicated disposable cache under the existing user profile and verified before it is offered, then verified again immediately before execution.
- The updater never reads commands, executable URLs or release selections from the Palisade web renderer.
- Downgrades and invalid or unsigned manifests are refused.

The Ed25519 key pair authenticates Palisade update metadata independently of a Windows Authenticode certificate. The private key is not stored in this repository. The source and release workflow require its public counterpart in `assets/update-public-key.pem`.

## Remaining risks

Windows SmartScreen and managed endpoint policies may reject unsigned installers. The updater does not bypass those controls. The application runs code served by the live Palisade website, so a compromised website remains a separate risk. A stolen release signing key could authorize malicious update metadata; protect it as a production release credential.

The release process is not independently audited. Before production distribution, verify Windows installation and update behavior on actual devices, test rollback and installer failure modes, review the Electron dependency chain, and establish an incident procedure for signing-key compromise.

Version 0.2.1 requires `release-v2.json` and rejects a missing or legacy-only manifest. The release workflow also emits the original `release.json` format so v0.2.0 can upgrade using its existing SHA-512 verifier. Those formats continue to be signed by the applicable bootstrap key. The v0.2.2 manual test-client bootstrap establishes the newly supplied key; older old-key executables cannot authenticate it automatically. GitHub release notes are bounded plain text for display and never determine executable commands or download locations.

The installer helper repeats SHA-256 verification after the application exits. It runs under the current user and Windows PowerShell execution policy, without an execution-policy override. A blocked helper leaves the running application open. NSIS exit failures are recorded for the next launch and trigger an attempt to reopen the installed executable. An interrupted or partially failed installation can require running the official installer manually; there is no automatic binary rollback. Cache cleanup never targets the browser partition.

Version 0.2.2 requires a v3 envelope binding the SHA-256 and exact GitHub download URL. It authenticates successor keys through predecessor-signed transitions, enforces activation versions and retains the packaged transition history as an append-only floor. An old envelope cannot bypass this requirement. Root-key loss and compromise need the recovery procedures in [UPDATE-SIGNING.md](UPDATE-SIGNING.md).

Custom window controls are rendered in a separate, local, sandboxed WebContentsView. Its isolated preload exposes only minimize, maximize/restore, close and sanitized window-state updates. The main process validates the exact sender, main frame, local file URL and action before handling a command. The hosted Palisade view has no preload or privileged control bridge. Local control views cannot request device permissions or navigate externally.
