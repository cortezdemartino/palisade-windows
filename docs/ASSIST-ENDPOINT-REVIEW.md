# Windows Assist endpoint review packet

Status: **Planned / Blocked**, awaiting the explicit owner and independent architecture decision in [project-palisade#33](https://github.com/cortezdemartino/project-palisade/issues/33). The full-scope request authorizes preparation, not a waiver of that decision. No native executor, service, input hook, named pipe, clipboard broker or filesystem broker is installed or listening.

## Proposed boundary

Prefer a separately packaged per-interactive-user, non-elevated native process for eventual input and sensitive handle ownership. This isolates it from renderer compromise and permits a renderer-independent watchdog, consent and emergency stop. It adds a binary/IPC/update trust boundary, so the choice of language/library, binary-integrity mechanism and code-signing policy needs review before implementation. Authenticode is separate from existing Ed25519 update metadata.

The Electron renderer remains sandboxed and cannot obtain raw IPC, command execution, OS handles, filesystem paths, clipboard access or authority from v1. Main should broker only fixed reviewed operations. A future local channel must authenticate the interactive SID, parent process/instance, endpoint challenge and session; use restrictive channel ACLs and anti-replay generation/nonces; refuse cross-user callers and generic commands. No unauthenticated inbound socket or persistent daemon is acceptable.

The endpoint must verify independently issued proof and device/peer possession, plus its own trusted local consent. Backend relay/session assertions alone must not authorize input. Issuer keys, endpoint enrollment and trust storage are unresolved companion/security decisions. The injectable verifier in `AssistLease` is a test seam, not that authority. No default verifier, renderer-supplied approval boolean or update-signing key is reused.

## Decisions required before executor implementation

- Endpoint identity enrollment, issuer/device trust, key separation and rotation/recovery.
- Authenticated local IPC design, process/SID binding, binary integrity and parent-death behavior.
- Separate native host consent for mouse, keyboard, each clipboard direction and each file direction; accurate identities and absolute expiry; no saved Allow Always.
- Native-only emergency stop and watchdog, release-all ordering, queues, flood and loss behavior. No renderer/backend dependency for stopping.
- Current display/source generation and physical-pixel/DIP/rotation scope. Window-only viewing must not imply enforceable window-only input.
- Keyboard layouts/IME and reviewed shortcut restrictions; respect UIPI, UAC, protected desktops and other users' sessions. Never send secure attention or unlock/elevate.
- Clipboard format/image decoding, no outside-session monitoring/history, browser gesture limits, confidentiality and denial behavior.
- Handle-based file selection/quarantine/final commit, reparse/TOCTOU/collision handling, quotas, scanning/MOTW and no execution.
- Standard authenticated data transport and cryptographic implementation; independently authenticated peers; no plaintext or media-key reuse.
- Disposable physical Windows test devices, independent security review, measured workloads, and separate release/activation approval.

## Implemented preparation and limits

`lease.cjs`: independent-verifier seam, exact binding, separate in-memory consent, 15-minute maximum, expiry timer, clock rollback, sequence/rate checks and terminal revocation. `input-model.cjs`: content/source geometry and key/button release plans only. `data-model.cjs`: bounded payload/file metadata and stream-integrity/cancellation helpers only. None is imported by a production operation handler. The v1 bridge reports `nativeEndpoint: absent`, `localRevocation: false`, `emergencyStop: false`.

`capture.cjs` and the trusted picker are component implementations behind a false build gate. Electron 44.7.2's permission ambiguity is a separate blocker in [Windows #3](https://github.com/cortezdemartino/palisade-windows/issues/3). Do not enable capture or approve empty media lists to work around it. A renderer termination mechanism was exercised, but actual capture termination was not established through a safe permission path.

## Review and verification procedure

Review the exact Windows checkpoint and [threat model](ASSIST-THREAT-MODEL.md), reconcile [the proposed handoff](ASSIST-CONTROL-HANDOFF.md) with the companion service, and record approve/reject plus named permitted repository/components in #33. Resolve findings before writing an executor. Then test local controls, frozen-renderer stop, all key/button releases, loss/expiry/replay/flood, cross-user/session/UIPI/secure-desktop restrictions and file/clipboard negatives on disposable devices. Unit state simulations are not those tests.

No review packet, issue comment or green CI run grants approval by itself. No installation or production activation follows automatically from approval to implement.
