# Security model

The Windows client is a desktop wrapper around the official Palisade web application. Message encryption and device verification remain the responsibility of the Matrix SDK and the Palisade service.

## Desktop boundary

The Electron renderer is sandboxed, runs without Node integration or a page-accessible preload bridge, and can navigate only within `https://palisade.onyxops.org`. External HTTPS links require confirmation before opening in the system browser. Non-HTTPS schemes, camera access, screen capture and privileged device APIs are blocked. Standard TLS validation remains enabled.

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

Custom window controls are rendered in a separate, local, sandboxed WebContentsView. Its isolated preload exposes only minimize, maximize/restore, close and sanitized window-state updates. The main process validates the exact sender, main frame, local protocol URL and action before handling a command. The hosted view has an isolated, one-way unread observer with no page-accessible privileged bridge. Local control views cannot request device permissions or navigate externally.

## Version 0.2.4 behavior

The client polls release metadata through the existing v3 verifier, including pinned-key/transition checks and stable-version comparison. It never downloads or starts an installer during a poll. Downloads and installation remain in the confirmed manager workflow, which rechecks its signed metadata and installer. Timer cleanup aborts pending checks on shutdown. The older internal installer-helper description above does not describe the current manager handoff.

The remote preload reads only rendered workspace/direct-message unread labels. Its unread observer exposes no functions to the web page and reads no browser storage, credentials, message text or Matrix APIs. Its one-way IPC accepts bounded counts only from the exact hosted main frame at the Palisade origin. Preview/invitation counts are ignored; initial synchronization is silent. The main process owns the icon and rate-limited generic notifications. Read-only DOM observation requires no website, backend or Matrix changes.

## M4G-WINDOWS permission preparation

Historical preparation: the initial missing web contract blocked Assist APIs and native capture grants. The current session policy rejects same-origin subframes, missing main-frame provenance, conflicting origins and stale/destroyed hosted contents. Media checks now grant audio only; camera/unknown checks remain denied. Display requests explicitly return null without enumerating sources. No new privileged renderer bridge, input capability or secret access exists. See [ASSIST-INTEGRATION.md](ASSIST-INTEGRATION.md) for the pinned contract audit and acceptance limits.

## M4G-WINDOWS-FULL candidate

The current v1 Assist API describes capabilities only and is accepted by the pinned actual web adapter. Capture remains disabled; empty media permission requests are denied because Electron 44.7.2 cannot distinguish modern display from legacy desktop/tab capture there. No input, clipboard, files or generic native IPC route exists. Isolated lease/planning/data helpers confer no production authority. See the current [integration guide](ASSIST-INTEGRATION.md), [threat model](ASSIST-THREAT-MODEL.md) and [endpoint review packet](ASSIST-ENDPOINT-REVIEW.md); earlier missing-v1-contract notes are historical.

## Inactive native guardian

The [guardian foundation](ASSIST-NATIVE-GUARDIAN.md) adds current-user DPAPI identity, strict Ed25519 issuance diagnostics, bounded inherited pipes, parent/user/session checks and an independent watchdog. It accepts no current Assist authority and is not started by production main/preload. Its binary hash manifest is inside the integrity-protected ASAR; native dependency audit and package verification are required before signing. Identity corruption fails closed without automatic reset. Directory replacement races by same-user malware, initial input-desktop qualification and eventual held-key crash recovery remain unqualified. No remote input, capture, OS clipboard or file transfer is activated.

## Current inactive native and capture components

Canonical stream control v2 is pinned at companion c0778935a6a08303bf7a9d7de8f2fa75aa8ef536. Dormant native components now implement local consent, bounded input permits, short signed-lease polling and protected file handles. Input suspension invalidates native grants without renewing them or revoking unrelated resource permissions. A guardian/child shared kernel-object lifetime prevents a successor while either holder remains. The native default package still has sensitive features disabled.

The isolated capture provider is separate from the hosted permission policy. Main validates the selected source, app-owned DTLS peer certificates and native socket ownership; monotonic challenges bound frozen-peer teardown. The hosted page receives no signaling or general native capability. App-owned window capture is tested, but this does not qualify cross-client E2EE or establish a firewall guarantee.

The current description bridge continues to report no native endpoint/capture. Production enrollment and page/gateway wiring remain incomplete. The inactive native v1 bridge validates exact messages and live main-document ownership, admits safety operations under load and rejects stale replies/events. Its real Electron test uses a fake coordinator and cannot authorize OS work. No production startup enables the surface.

The issuer now requires release-pinned or independently confirmed trust. Legacy first-use records are preserved without promotion. Qualified DPAPI records bind origin, key, fingerprint and method; changed/corrupt state fails closed. No production release fingerprint is configured. Endpoint statements are purpose-labelled; raw challenge signing is removed. See [trust and recovery](ASSIST-NATIVE-GUARDIAN.md#issuer-trust-and-recovery), the current [threat model](ASSIST-THREAT-MODEL.md), [capture boundary](ASSIST-CAPTURE.md) and [integration gates](ASSIST-INTEGRATION.md).
