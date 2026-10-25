# Project Palisade: unblock full Windows Assist integration

Writable repository: cortezdemartino/project-palisade. Inspect cortezdemartino/palisade-windows read-only. Work on main, preserve concurrent changes, and create no branches or pull requests. Do not deploy, enable production features, publish Windows releases or implement/install Windows native input.

Parent enhancement: project-palisade#18. Companion dependency: palisade-windows#4. Native endpoint review: project-palisade#33.

## Audit first

Refresh main and record the exact revisions. The last inspected published web revision is 1e23a6ef243934fce10dc65255c0273c1afe814f. Its Assist contract is still view-only. Windows has the real v1 description bridge, with capture unavailable and native endpoint absent. Do not repeat or replace that bridge work.

Read repository instructions, the risk-based testing policy, docs/44, docs/45, docs/46, services/admin/src/{assist,calls,media,voiceAuthz}.ts, apps/web/src/platform/desktopAssistBridge.ts, the Assist feature modules, directCallController.ts, sdkVoice.ts and annotations.ts. Read the Windows ASSIST-CONTROL-HANDOFF.md, ASSIST-ENDPOINT-REVIEW.md and ASSIST-THREAT-MODEL.md at the exact Windows revision you record.

## First deliverable: a canonical integration contract

Implement a separately versioned full Assist contract and real service validation for the following. Preserve v1's exact description-only interface and existing browser/view-only behavior.

1. Private, single-redemption invitations bound to the selected authenticated person and device. Users do not copy keys. Define safe retry/concurrent-redemption behavior.
2. Independently verified participant/device/endpoint proof, scoped grants, host consent, native acknowledgment, generations, nonces and replay protection. Select supported identity/crypto APIs and document the trust chain. No renderer assertion, media token or server identity string can substitute for that proof. Keep key families separate and do not export Matrix private keys.
3. An authenticated original absolute session deadline that a later grant cannot extend. Honor the existing fifteen-minute maximum and authoritative revocation. Reconnect/unlock must not restore control.
4. Separate viewing, pointer/annotation, mouse, keyboard, shortcut, clipboard-direction and file-direction permissions. Include granular revocation and cancellation semantics.
5. Bounded capture request correlation, pending-picker cancellation, source/status generations, late-result refusal and local-stop reporting. Do not broaden Electron permission rules to bypass the Windows runtime blocker.
6. Exact types/schemas, allowed fields, payload limits, rates, ordering, timeouts, state transitions and typed errors. Supply executable positive/negative JSON fixtures that Windows can consume without copying a second protocol.

The Windows offline helper now requires bind(envelope, expectedBinding, originalSessionDeadline). It exposes read-only authorization checks, per-capability AbortSignals, granular local revoke and terminal full cancellation. These are private helper APIs, not names the network protocol must adopt. Its verifier is synthetic in tests and is not a real issuer or device-authentication implementation.

## Implement the companion responsibilities

Add the applicable backend admission/revocation/signaling and web adapter/UI behavior using the canonical contract. Keep actual control unavailable until the approved native endpoint independently confirms authorization and local consent. Guest requests cannot approve themselves.

Preserve MatrixRTC/LiveKit media E2EE. Use a reviewed authenticated encrypted data path with device/session/fingerprint binding; do not enable general canPublishData, reuse media keys ad hoc, add plaintext fallback or store private transfer contents on servers. Keep release/control traffic independent of bulk-file backpressure.

Implement bounded guest mouse/keyboard/shortcut/virtual-keyboard events, focus-loss release, source generations, independent clipboard gestures/directions, file offer/accept/progress/cancel/integrity and supported browser save behavior. Reuse the existing annotation architecture with Assist-specific grants. No remote filesystem browser, silent overwrite or automatic execution.

Publish the first verified contract/types/fixture commit as soon as it is usable for Windows integration; continue the remaining web work in coherent slices. Do not postpone the integration handoff until every UI convenience is finished. If identity or transport cannot be established safely, provide exact findings and a concrete reviewed proposal while completing independent parsers/service negatives. Never invent successful verification.

## Review, verification and handoff

Review the Windows native design from the companion boundary and record findings under #33. Do not record owner approval or close that gate on the owner's behalf. Backend code does not authorize native implementation or activation.

Run the applicable High-risk checks, real service/Matrix/LiveKit negatives and browser acceptance. Cover wrong identities/devices/endpoints, forwarded invitations, duplicate redemption, expired/replayed/revoked grants, guest self-approval, stale input/source generations, reconnect restoration, unauthorized clipboard/files, malformed/flooding data and transfer cancellation/integrity.

Mocks prove component behavior only. Real Windows and cross-client acceptance remains Not Verified until executed with the Windows implementation. Keep unsupported production controls disabled and update practical user/admin/operator guides and the canonical Windows handoff.

Commit verified slices on main, push normally, report exact-SHA CI, and update #18/#4 and relevant assigned interventions. Return the final web SHA, protocol version, shared fixture paths/hashes, proof-verification requirements, exact Windows APIs/messages to implement, actual test evidence and remaining gates. Use Implemented / Experimental / Planned / Blocked / Not Verified accurately.

No deployment, production activation, endpoint installation or partial Assist release is authorized by this handoff.
