# Assist control and data handoff proposal

Status: **Planned contract, not a production protocol.** Windows components implement only offline admission, planning and bounded-stream helpers. No endpoint, input, clipboard or file channel consumes these messages. The compatible current web revision is `6c7ab894862ca3767b1239594e37c322e96ff2c4` for the description-only v1 bridge, not for the proposed operations below.

Update, 2026-10-25: companion ee9c2763f19e62b405bbf9a9c97f187bc5a1291a now contains the canonical palisade.assist.control v1 contract in docs/47, service grants, shared fixtures and data/input components. It is pairwise and capped at fifteen minutes. The conceptual Windows types below are not a competing wire format and must be reconciled with that actual contract. The owner has approved the native architecture and the [stream/group session amendment](ASSIST-STREAM-SESSIONS.md): viewer requests or streamer invitations, host-selected permissions and up to two hours, terminal on stream end. That amendment needs a compatible versioned companion contract; do not relax legacy v1 validation or trust its published fixture-only issuer key in production.

Do not append operations or capability fields to `palisade.assist.bridge` v1. Its three members and seven-field report remain unchanged. The companion team must reconcile this proposal with a new canonical service/data contract and supply executable shared fixtures before integration. Track [Windows #4](https://github.com/cortezdemartino/palisade-windows/issues/4) and [parent #18](https://github.com/cortezdemartino/project-palisade/issues/18).

## Recommended invitation and access-pass behavior

Engineering defaults use an in-app invitation addressed to one authenticated person/device. The invitation can establish one session; the accepted session has separate scoped grants for viewing, mouse, keyboard, clipboard directions and file directions. Users do not copy raw keys. Consent shows the actual participant/device, permissions, deadline and local stop; automatic key exchange never makes access invisible.

An invitation or forwarded token alone cannot authorize a different device. The native endpoint must independently verify host approval and guest-device possession using the reviewed Matrix/device trust contract, plus its own current local consent. Reconcile this proof with supported identity APIs; do not invent a second identity authority or accept renderer assertions. Use standard reviewed cryptography and separate endpoint/session keys from updater-signing keys. Native endpoint private keys stay in per-user OS-protected storage and outside the hosted renderer.

Invitation redemption must be atomic and bounded; retries must not establish duplicate sessions. Grants retain the original Assist deadline, endpoint/session/device bindings and replay/revocation scope below. Expiration, local stop and connection uncertainty end affected authority. A refreshed transport or unlocked desktop cannot restore control without fresh valid authorization and consent. Single invitation redemption is distinct from validating every action during the resulting session.

These defaults are review input, not committed web/backend behavior. Shared fixtures must cover forwarded invitations, wrong devices/endpoints, duplicate redemption, replay, permission expansion, expiry, revocation and attempted restoration. Companion source remains read-only in this assignment.

## Proposed authority types

The following types are review input. They are not trusted JSON accepted from the current page. A future independently implemented verifier must authenticate the envelope and derive these fields, including the endpoint/peer proof. An object claiming `verified: true`, a bridge report, a media token or the Palisade domain cannot substitute for that verifier.

```ts
type Binding = {
  sessionId: string;
  hostAccount: string; hostDevice: string;
  guestAccount: string; guestDevice: string;
  endpointId: string;
};
type ScopedGrant = Binding & {
  grantId: string; nonce: string; generation: number;
  issuedAt: number; expiresAt: number;
  capabilities: Capability[];
};
type Envelope = {
  sessionId: string; grantId: string; nonce: string;
  generation: number; sequence: number;
};
type Capability =
  | 'view_screen' | 'send_pointer' | 'annotate_screen' | 'clear_annotations'
  | 'control_mouse' | 'control_keyboard' | 'send_shortcut'
  | 'clipboard_send_text' | 'clipboard_receive_text'
  | 'clipboard_send_image' | 'clipboard_receive_image'
  | 'file_send' | 'file_receive';
```

Endpoint IDs, signed envelopes, issuer trust, device possession, key enrollment and network revocation delivery are **not implemented**. `AssistLease` requires a verifier callback, checks the exact six-field binding, limits lifetime to 15 minutes, then separately requires local consent. It cannot mint a grant. Its tests use an explicitly synthetic verifier. Consent on this object is not a trusted native consent UI.

The service must supply the original absolute Assist deadline; a later grant must not extend it. The helper now requires `bind(envelope, expectedBinding, sessionDeadline)`: the third argument must come from independently authenticated original session context, not from the grant being admitted or a page field. Missing/invalid deadlines and grants exceeding that deadline are refused. This is a local helper API, not a new web protocol field.

Consent, read-only authorization checks and message admission all check current time; invalid/rolled-back clocks and expiration terminate the lease. The helper rejects a grant if verification was interrupted by local revocation. Idle expiration also terminates without another packet. Generation changes and transport loss still require the future adapter to revoke the old lease; no reconnect restores consent.

Local `revokeCapabilities()` removes the requested permissions before cancellation listeners run. `signalFor(capability)` supplies an irreversible per-capability AbortSignal, while `signal` ends the whole lease. Consumers must use the scoped signal and `assertAuthorized(capability)` to stop stalled work without consuming a packet sequence. Revoking one direction does not grant or restore another. Full revoke invalidates authority and signals cancellation before the cleanup callback, including when that callback fails. This proves in-process state/cancellation only; native key release, independent emergency stop and network revoke authenticity remain unimplemented.

## Proposed bounded payloads

Examples below omit the common envelope for readability. Names are provisional; no current web API sends them.

```json
{"kind":"pointer","sourceGeneration":3,"x":0.5,"y":0.25}
{"kind":"key","code":"KeyC","down":true}
{"kind":"button","button":"left","down":false}
{"kind":"clipboard-offer","format":"text/plain","size":42,"digest":"<sha256>"}
{"kind":"file-offer","transferId":"<random-id>","name":"report.txt","size":4096,"sha256":"<sha256>"}
{"kind":"file-accept","transferId":"<same-id>"}
{"kind":"file-cancel","transferId":"<same-id>"}
```

The canonical design must specify exact allowed keys, encoding, maximum serialized sizes, sequence scope, acknowledgments, chunk identifiers and capability direction. Do not treat examples as an approved wire format. New protocol versions need downgrade/incompatible-version tests and current v1 fallback.

## Native input integration points

`input-model.cjs` translates a viewer content rectangle into a validated source rectangle, including negative monitor origins and 0/90/180/270 rotation. The web viewer must provide its real content rectangle after zoom, pan and letterboxing. The native endpoint must derive current display bounds, pixel/DIP mapping, source generation and allowed region itself; remote values cannot authorize a region. DPI changes and source switches invalidate the old generation.

The key/button ledger plans releases in reverse key order and clears all state. It does not call SendInput. Secure attention combinations, OS/meta keys and Alt+Tab are rejected by the current planner. The small shortcut list is a proposal, not an approved production allowlist. Physical scancodes, virtual keys, Unicode/IME, AltGr/dead-key interpretation, repeat timing and a guest virtual keyboard remain blocked on the approved native executor and companion UI. Do not advertise those as working.

Proposed state contract: inactive → initialized → independently authenticated → session-bound → consent-pending → viewing-active → control-requested → control-authorized → control-active. Control may suspend/revoke, and any security uncertainty terminates the lease. After suspension, fresh proof and local consent are required. No state transition by itself injects input. The helper implements admission/consent/termination, not this full endpoint state machine.

One controller device may hold a native input lease. Independent local stop must reject new packets before releasing every held key/button, including when the renderer is frozen. Watchdog/parent death, Windows lock, user switch, sign-out, secure desktop, sleep, source loss and transport loss must execute that path in the native process. These OS behaviors require real tests before any executor is enabled. Window-only capture must not imply window-confined input; desktop control needs accurately described broader consent if confinement cannot be enforced.

## Clipboard and files

The current helpers validate bounded UTF-8 plain text and PNG envelope dimensions; reject HTML/other formats; deduplicate within an explicitly enabled in-memory window; and clear that window on revocation. They do not monitor/read/write the OS clipboard. PNG header validation is not a full safe image decoder. Directional grants, explicit native consent, browser user gestures, rate quotas, image decoding and actual OS access remain integration requirements. No secret detector or clipboard history is claimed.

The file helper validates a basename, 1 GiB file ceiling, lowercase SHA-256, Windows reserved names, traversal/ADS characters and misleading direction controls. It refuses unauthorized work before opening or polling the source iterator. It receives chunks of at most 64 KiB into a caller-provided quarantine Writable, copies each chunk, waits for writes, rechecks positive authorization and cancellation, verifies final length/hash, and cancels stalled reads/writes through an AbortSignal. Destination failure interrupts a stalled source; completion uses Node's stream lifecycle handling. It takes no path, creates no file and never executes content. Hash integrity is not sender authentication or encryption.

The future broker must open sender-selected handles locally, send opaque transfer IDs rather than paths, require recipient acceptance and a trusted local save decision, apply aggregate/concurrent quotas, use safe private quarantine handles and verify before a collision-safe final commit. Atomic commit, junction/reparse/race protection, Mark-of-the-Web/security scanning, receipts, retry/resume and destination policy are **not implemented** by this stream helper. Resume requires a newly validated grant and an agreed chunk map/digest policy; cancellation never implies permission to resume. Directory traversal and automatic file execution remain unsupported.

## Transport and cryptographic gate

No network transport or custom encryption scheme was implemented. Review standard authenticated end-to-end transport and content encryption, with keys separated from MatrixRTC media. Endpoint/device possession and independently authenticated peer/session binding must be established outside the compromised renderer. Neither TLS to a server nor media E2EE proves control/data confidentiality. Do not enable general LiveKit `canPublishData`, send secrets through v1, reuse media keys ad hoc, persist private files on a server or add a plaintext fallback.

Existing `annotations.ts` uses `com.onyxops.palisade.annotation` v1, Olm-encrypted to-device messages and call/device/presenter binding, with up to 240 stroke points. Reuse that actual implementation when the Assist service explicitly grants it; no competing drawing protocol is added here. Pointers/annotations must remain separate from native mouse input.

## Companion implementation checklist

1. `platform/desktopAssistBridge.ts`: preserve v1; define any new approved protocol separately with parser/timeout/cancellation tests.
2. `features/assist/assistCapabilities.ts` and `useDesktopAssist.ts`: resolve each independent capability against service policy, actual endpoint status, live session and host consent. Never show enabled unsupported controls.
3. `calls/directCallController.ts`: carry authoritative deadline, generation, participant/device bindings and revocation through teardown; prevent late publication/actions.
4. `matrix/voice/sdkVoice.ts`: retain MatrixRTC/LiveKit E2EE and normal capture path; do not authorize input from media grants.
5. Service Assist/media authorization: add independently reviewed endpoint enrollment/proof and granular grant/revoke routes. Host/guest/device checks, expiry, restart, wrong role, replay and flood tests must use the real service.
6. Guest UI: generate validated content-relative events; request native control distinctly; respect browser clipboard gestures; select/download files through approved browser APIs; display identities, capability directions and explicit stop/leave states.
7. Add fixtures shared with Windows admission/stream/planner tests, real encrypted data/media observers, all four cross-client cases, NAT/TURN/loss/restart tests, and exact-SHA evidence before feature-gate activation.

Expected errors include incompatible protocol, invalid payload, unavailable endpoint, absent consent, binding mismatch, expired/revoked grant, stale source, replay, rate/quota exceeded, cancelled transfer and integrity failure. Their final names/codes must be chosen in the canonical contract, not inferred from local JavaScript exception text. No free-text error may carry secrets, window titles, clipboard or file contents.
