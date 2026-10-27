# Windows Assist integration

Component checkpoint, October 27, 2026. **Full Assist: Blocked.** Windows now includes dormant native input, local consent, independent signed-lease polling and resource brokers. They are not connected to the hosted application. Production still reports capture unavailable and nativeEndpoint absent. This is not a full Assist release candidate.

## Compatible contracts

The description-only `palisade.assist.bridge` v1 remains unchanged. Its three members and seven report fields cannot authorize native work. Legacy control v1 retains its fifteen-minute ceiling.

Windows verifies the canonical stream/group control v2 contract at companion commit `c0778935a6a08303bf7a9d7de8f2fa75aa8ef536`. Exact source, schema and fixtures, with Git blob and LF-normalized SHA-256 provenance, are retained under tests/fixtures. Grants bind both accounts/devices, helper DTLS fingerprint, endpoint, room, stream SID, generation and an immutable deadline up to two hours. Separate twenty-second signed leases determine which grants may act. Native polling uses the endpoint key rather than Matrix tokens or page cookies.

The companion source remains read-only. Companion ae89e414ec71a193094ef7cbee5ad678418ff6f6 implements the web native adapter, shared-certificate input peer, virtual keyboard, signaling and encrypted capture publication. Its browser acceptance uses a mock endpoint and does not qualify Windows. Native bridge v1 remains the unchanged docs/49 contract from 8f9c2a45da7017ee36a9567f76413da53bd76075; its exact validator blob is 62f29f433dbc20de881108b635a97eee3bde6827. The earlier service authority fixes were independently source-reviewed. Official companion CI evidence remains required. Resource transport/actions, helper authority, relay configuration and production pin setup remain dependencies. See [the companion handoff](ASSIST-COMPANION-HANDOFF-PROMPT.md).

## Status

| Component | Classification | Evidence and remaining work |
| --- | --- | --- |
| Existing desktop, updater and description bridge | Implemented | Regression tests; app identity, updater trust and persistent profile preserved |
| Canonical stream v2 verification | Implemented, inactive | Shared TypeScript/Rust signatures, fixtures and negative tests |
| Native consent, acknowledgement and lease polling | Experimental | App-owned Win32 consent and fixed-origin HTTPS; live enrollment/service acceptance not executed |
| Mouse, keyboard and bounded shortcuts | Experimental | SendInput adapter, source/session/UIPI checks and owned-state cleanup; real input not qualified |
| Guardian/child execution | Experimental | Private pipes, mirrored intent, fenced cleanup, heartbeat and probe tests; physical crash/lock recovery not qualified |
| Clipboard text | Experimental | Separate directions, positive authority, manual copy and cancellation; OS/peer wiring pending |
| Native files | Experimental, inactive | Runtime-owned publisher requests, exact grant/direction/offer binding, independent cancellation, shared quota and protected staging; temp NTFS tests pass, native file confirmation is unqualified |
| Native capture | Experimental, inactive | Existing picker, isolated provider, owned DTLS video, certificate/socket checks and app-owned window pixel/stop fixtures; canonical web publication adapter and acceptance pending |
| Native bridge | Implemented, inactive | Exact two-member native v1 surface, strict messages, live-document witness, private event nonce, bounded requests, cancellation and terminal cleanup scopes; real Electron fixture uses a fake coordinator |
| Owned remote DTLS gateway and coordinator | In progress / Blocked | Enrollment tuple, actual peer/source binding, resource actions and live native IPC still need integration and qualification |
| Virtual keyboard, full toolbar and annotations | Experimental / Blocked | Companion web controls exist; Windows endpoint integration and real acceptance remain |
| Complete cross-client Assist | Not Verified | No installed endpoint or real encrypted control/transfer session exercised |
| Activation/release | Blocked | Integration review, disposable-device qualification and separate owner approval required |

## Security boundaries

The hosted renderer remains sandboxed with context isolation and no Node integration. Current native components are internal and unlaunched. No generic native command, filesystem, clipboard or input bridge exists. Existing scoped microphone/notification permissions remain. Generic display permission stays denied because Electron's ambiguous legacy path could bypass a hosted picker. A capability boolean cannot enable it.

Endpoint identity uses a separate current-user DPAPI-protected Ed25519 key. Updater and Matrix keys are never reused. The issuer now requires a release pin or native out-of-band administrator confirmation. Legacy issuer-v2.dpapi first-use records cannot qualify trust and remain untouched. The versioned qualified record binds origin, key, fingerprint and method under DPAPI. No production issuer fingerprint is configured. See [native trust and recovery](ASSIST-NATIVE-GUARDIAN.md#issuer-trust-and-recovery).

Endpoint acknowledgements and lease requests use the exact docs/49 purpose-labelled statements. Shared vectors verify all four statement encodings and reject cross-purpose substitution. The old raw-challenge signer has been removed from both normal and probe IPC. Enrollment proof and transport signing must be owned by their corresponding native coordinators before exposure; no generalized signer is available.

Sensitive operations need current exact authorization, a fresh lease, local consent and the verified helper connection. Input permits bind one action, instance, epoch and source generation for at most 100ms. Expiry invalidates idle held-input authority. Cleanup fences and reaps the child before guardian release. Probe mode cannot inject input, including with sensitive features compiled.

Clipboard defaults Off and supports bounded text only. Sync additionally requires a negotiated agreement. Pause/revocation cancels in-flight automatic work. Images, rich formats, transfer resume and unrestricted drag/drop are unsupported at this checkpoint. No remote path or automatic file execution is exposed.

## Verification and next integration

The executable sequence, dependencies and release gates are in [ASSIST-RELEASE-PLAN.md](ASSIST-RELEASE-PLAN.md). The publisher-side resource adapter does not authorize a Windows helper's local files or clipboard; that role requires its own finalized native authority.

See [isolated capture](ASSIST-CAPTURE.md), [native components](ASSIST-NATIVE-GUARDIAN.md), [test results](TEST-RESULTS.md), [threat model](ASSIST-THREAT-MODEL.md) and [milestone report](M4G-WINDOWS-REPORT.md). A passing build or probe does not qualify physical input, issuer enrollment, encrypted viewing or cross-client transfer.

Owner architecture approval is already recorded under [project-palisade#33](https://github.com/cortezdemartino/project-palisade/issues/33#issuecomment-6399951819). Remaining work is tracked in [Windows#1](https://github.com/cortezdemartino/palisade-windows/issues/1), [#3](https://github.com/cortezdemartino/palisade-windows/issues/3), [#4](https://github.com/cortezdemartino/palisade-windows/issues/4) and [#5](https://github.com/cortezdemartino/palisade-windows/issues/5). Window-bounds persistence remains queued after Assist testing readiness.

No installer was run, personal clipboard accessed, real input injected or release published during this component checkpoint. The packaged native build has no sensitive Cargo features.
