# Security model

The Windows client is a desktop wrapper around the official Palisade web application. Message encryption and device verification remain the responsibility of the Matrix SDK and the Palisade service.

## Desktop boundary

The Electron renderer is sandboxed, runs without Node integration or a preload bridge, and can navigate only within `https://palisade.onyxops.org`. External HTTPS links require confirmation before opening in the system browser. Non-HTTPS schemes, camera access, screen capture and privileged device APIs are blocked. Standard TLS validation remains enabled.

The desktop client uses its own persistent browser partition. It does not use Windows DPAPI or the TPM to independently protect Matrix tokens or cryptographic material. Signing out or revoking a session remains essential after device loss.

## Update boundary

- Update checks use the public GitHub Releases API for `cortezdemartino/palisade-windows`.
- Only stable, non-draft releases with a numeric `vMAJOR.MINOR.PATCH` tag are considered.
- The `release.json` asset must contain an Ed25519 signature from the key embedded in this application.
- The signed data fixes the version, installer filename, size, SHA-512 checksum and channel.
- Downloads use an exact GitHub release path. Redirects are limited to HTTPS GitHub release-asset hosts.
- The installer is downloaded to a dedicated temporary directory and verified before it is offered, then verified again immediately before execution.
- The updater never reads commands, executable URLs or release selections from the Palisade web renderer.
- Downgrades and invalid or unsigned manifests are refused.

The Ed25519 key pair authenticates Palisade update metadata independently of a Windows Authenticode certificate. The private key is not stored in this repository. The source and release workflow require its public counterpart in `assets/update-public-key.pem`.

## Remaining risks

Windows SmartScreen and managed endpoint policies may reject unsigned installers. The updater does not bypass those controls. The application runs code served by the live Palisade website, so a compromised website remains a separate risk. A stolen release signing key could authorize malicious update metadata; protect it as a production release credential.

The release process is not independently audited. Before production distribution, verify Windows installation and update behavior on actual devices, test rollback and installer failure modes, review the Electron dependency chain, and establish an incident procedure for signing-key compromise.
