# Complete Windows Assist delivery plan

Baseline: Windows `7fcae472c041b1569233880cff96ae0a691bc7d1`, exact-SHA CI [40602184816](https://github.com/cortezdemartino/palisade-windows/actions/runs/40602184816) passed. Companion inspected read-only at `ae89e414ec71a193094ef7cbee5ad678418ff6f6`. Native bridge v1 remains the canonical docs/49 contract. Existing evidence: 143 application tests, 69 native tests in each configuration, process probes, NSIS/package checks and 14 actual Electron transport fixtures with a fake input sink. These are component results, not full Assist acceptance.

## Delivery commitment and ownership

Complete all Windows features below, including their live wiring, user interfaces, negative tests, packaging and documentation. No placeholder, mock native endpoint, permanently disabled route or component-only result completes a feature. Missing companion contracts block the affected feature; they do not remove it from scope.

Work on the existing Windows main checkout. No branches, worktrees, pull requests, force pushes or discarded concurrent work. The companion remains read-only here; Claude receives the exact versioned requirements in [the companion handoff](ASSIST-COMPANION-HANDOFF-PROMPT.md). Reuse the existing bridge, native runtime, grant/lease verifier, capture and resource brokers. Do not create a competing service protocol or duplicate grant registry.

Apply the official conservative High-risk verification policy. Companion docs/42 explicitly replaces disabled Actions with equivalent exact-candidate local package, real-service, Chromium/Firefox and ciphertext checks. Honor that exception without requiring CI re-enablement. Windows Actions remain enabled and must pass on the exact pushed candidate. Actual OS/cross-client/security/performance gates remain independent of either automation path.

Architecture approval and independent reviewer authorization already exist. No further design decision is requested from the owner. Installation, native activation, physical input testing on approved disposable devices, production enablement and publication retain their existing separate approvals. Never use the owner's personal desktop for remote-input acceptance. A qualified candidate must be concrete and reviewable before requesting final approval.

## Feature completion ledger

Every row remains open until its implementation and real acceptance evidence are recorded. Classifications are Implemented, Experimental, Planned, Blocked or Not Verified. A feature's transport, native operation and permission UI must all agree before it can be advertised.

| Feature | Remaining Windows delivery | Completion evidence | Companion dependency |
| --- | --- | --- | --- |
| Stream/group requests and invitations | Bind viewer requests and streamer invitations to specific authenticated accounts/devices, explicit capabilities and the original deadline up to two hours | Specific-user/group flows, atomic controller replacement, deadline and stream-end revocation; group voice continues | Existing stream v2 service/web, final compatibility fixtures |
| Enrollment and issuer trust | Connect qualified issuer, installation key, native-confirmed publisher tuple and revocation to the live coordinator | Actual native Allow/Deny/cancel; wrong issuer/device/key/session; re-enrollment invalidates old work | Existing labelled proof/ack/lease contract; production fingerprint under project#47 |
| Screen/window viewing | Connect native picker, previews, operation/source generations and owned video to existing encrypted publication | Monitor/window selection, cancel/change/loss, privacy boundary and real encrypted cross-client viewing | Existing capture adapter; exact source/SID trust semantics |
| Multi-monitor and source scope | Resolve native display/window handles, physical geometry, scaling, rotation and source lifetime; accurately describe control scope | Mixed DPI, negative origins, portrait/resolution changes, unplug/reconnect and moving/closed windows | Source-change/permission lifecycle contract |
| Mouse | Wire actual movement, clicks/double-clicks, down/up, drag, both scroll axes and owned-button cleanup | Real approved-device actions, coordinate accuracy, local priority, revoke/freeze/disconnect cleanup | Existing control v2 events; version any missing semantics |
| Keyboard and virtual keyboard | Wire physical keys, modifiers, repeats, permitted shortcuts, layout-aware behavior, bounded Unicode/text where supported, and guest on-screen controls | Real keys/modifiers/shortcuts, international/AltGr/dead-key/IME cases with documented limits; no stuck keys or protected-desktop bypass | Existing web keyboard UI/events; canonical extension for any absent text/keyboard semantics |
| Pointers and annotations | Reuse current stream annotation protocol and separate visual overlays from OS input | Pointer/laser/drawing/arrow/rectangle/ellipse/clear permissions; revocation; no native input from annotation messages | Existing channel annotations; explicit direct-call support rather than bypassing its current restriction |
| Clipboard | Wire independently authorized send/receive text and safe images, manual actions, optional explicitly consented synchronization, deduplication, limits and cleanup | Real Windows/web directions, gestures, image validation/size limits, loops/races, no access outside the session and immediate revocation | Canonical native resource actions/data plane, image formats and helper authority |
| Direct files | Wire selected handles, individual accept/reject, safe save, multiple files, encrypted chunks, progress/rate, timeout/cancel/retry, verified completion and fresh-authorized resume | Actual bidirectional transfers, integrity, quota/concurrency, traversal/reparse/collision/MOTW, cleanup, no overwrite or execution | Canonical native resource actions/encrypted formats, helper authority, receipts and resume policy |
| Drag/drop | Add visible local file-drop targets using the same selected-handle/send/receive consent flow | Actual supported client drops; no remote filesystem browsing or path exposure; cancel/revoke | Canonical web sender/receiver and file-drop integration |
| Host/guest controls | Connect actual toolbar states, participant/device identity, separate permission indicators, expiry, local suspend/revoke/stop and errors | Rendered and physical UI checks; unavailable capabilities cannot appear functional | Existing web toolbar plus final resource/status actions |
| Native endpoint and IPC | Add a typed live mode, bounded authenticated pipes, independent stop, generation checks and lifecycle ownership | Real process tests for malformed/replayed/cross-session/blocked IPC, EOF, parent/child death and backpressure | No new general page API; canonical bridge stays strict |
| Security transitions and recovery | Revoke on lock, user switch, sleep, source/authority loss; retain release-only cleanup; require fresh authority after recovery | Actual lock/UAC/UIPI/RDP/session/crash/network cases, emergency stop first; no unattended resume | Service/peer lifecycle and approved relay configuration |
| Quit, update and uninstall | One shared revocation/cleanup barrier before both app exit and update-manager spawn; preserve sessions/settings and installer verification | Active-session update, delayed/failed cleanup, pending files, installed upgrade and removal; no surviving input authority | Status semantics distinguish revoked from cleanup complete |
| Performance and privacy | Measure capture/input latency, CPU/memory/GPU/bandwidth, clipboard latency and file throughput under realistic conditions | Recorded hardware, versions, LAN/WAN/TURN, loss, high resolution, multiple monitors and voice/file concurrency | Working real companion transports |
| Distribution and documentation | Qualified build profile, exact-SHA High-risk verification, package/fuse/native integrity, signed updates and complete user/operator guides | Fresh candidate SHA/hash, all required gates, approved release signing/published asset verification and upgrade acceptance | Production issuer/enablement and joint acceptance approvals |

Images, authorized resume and drag/drop remain delivery items. They cannot silently disappear because the first implementation supports text or single-file transfer. Directory transfer is conditional in the original scope; do not expose unrestricted traversal or a remote filesystem browser. Any requested change to the accepted feature inventory must be explicit.

## Ordered implementation and verification

### W1. Establish live shutdown and cleanup ownership

Implement one application-owned lifecycle registry/barrier. Revocation is synchronous and independent of a pending ordinary request; completion waits for resource cancellation, fenced input release and child exit. New Assist work is refused once shutdown starts. Stop alone need not exit the app. Ending a child Assist leaves its parent stream/voice intact.

Route both `main.cjs` quit handling and `updater.cjs` manager launch through that barrier. The updater currently spawns its manager before calling app.quit, so a before-quit hook alone is insufficient. Flush existing profile storage at the correct point. Cleanup failure or protected-desktop delay postpones quit/update and exposes an actionable status; never kill the last cleanup holder to satisfy a timeout. Normal desktop operation with no Assist participant must retain existing behavior.

Verify with deferred/rejected cleanup, concurrent quit/update, new-work races, preserved state and zero manager spawn before cleanup. Use fake holders first; native child/process acceptance follows W2/W8.

### W2. Add the live native coordinator channel

Keep diagnostic guardian v1 and its 16KiB framing unchanged. Add a separately negotiated private live mode/broker with closed typed commands/results, bounded frame sizes, IDs, queues, deadlines and strict parent/image/SID/session/pipe checks. No generic invoke, executable command, arbitrary path, public signer, page consent boolean or credential extraction.

A reader/watchdog must set native revocation flags even while dialogs, HTTPS, dispatch or stdout block. Live EOF/watchdog/parent loss must revoke and retain cleanup rather than use the diagnostic process-kill/exit behavior. Give ordinary requests bounded capacity and reserve safety handling. Expose nonblocking revoke separately from cleanup/join completion. Probe/test mode remains non-authorizing even with native features compiled.

A direct guardian exit is not sufficient completion proof when a release-only child survives a crash. Retain authenticated cleanup-holder/process ownership evidence through every descendant's release and exit; do not substitute process-name polling or a timeout. Include parent/guardian/child crash combinations in the approved-device acceptance and document unavoidable combined-host-failure limits.

Wire enrollment and the existing runtime behind this channel. Keep SessionGate/LeasePoller as the sole grant registry; explicitly offered grants are validated/admitted through it without recreating a session per capability. Cancel stale documents and late dialogs/signatures by immutable operation generations.

Verify actual pipes, parent loss, malformed/oversized/duplicate/replayed frames, blocked operations/output, cancellation priority, stale completion and retained cleanup. No test shortcut may authorize production input or a production identity.

### W3. Connect source, consent and signed transport

Resolve each trusted picker choice to a native-owned source record with handle/owner lifetime, operation and generation. Bind it to enrollment, room, tentative publication SID and each session/grant. Recheck native source identity/geometry; revoke children before releasing/replacing a source. A page-reported SID never proves which source was published.

Current native geometry validates an exact monitor. Implement accurate selected-window mapping and explicit broader control consent where strict window-only input cannot be enforced. Do not map window-normalized points onto an entire monitor. Monitor bounds also do not confine keyboard shortcuts to that monitor; native consent must describe the interactive-desktop keyboard scope accurately. Preserve UAC, secure-desktop, cross-user and elevation boundaries.

Implement native grant consent/acknowledgement and the owned peer's exact labelled transport signature. Transport setup needs a narrow permit for the current controller, enrollment/source generation, local consent and a fresh covering active lease. It must not require peer_verified, since the signed answer precedes DTLS. Never temporarily mark a peer verified. Allow a bounded cancellable wait for the post-ACK lease poll. Sign only the actual app-owned answer fingerprint and grant-bound helper certificate.

After actual DTLS observation, await native verification and input_ready/idle warmup before admitting packets. Keep the original bytes for native strict decoding. Test signature-purpose substitution, forged source/helper/answer, lease races, stale grants and early packets through the real bridge/coordinator chain.

Sensitive native UI requires a native-owned guard for the entire dialog/confirmation lifetime. Immediately revoke input, await executor fence/reap and owned release before enabling Allow/Share/Save, and block replacement controllers, input acknowledgements, transport setup and automatic warmup while guarded. Keep Stop/Deny available while cleanup waits; neither the urgent reader nor the gate lock may wait on UI. Afterward require fresh input authority while preserving independent resources. An event marked isTrusted or a protected-window target check is not proof of local host intent.

The guard also covers capture/source selection and permission/settings changes. Validate and rate-limit requests before displaying prompts. Test overlapping confirmations, held Enter/Space/buttons, queued old input, delayed release and stale dialog completion; none may approve an operation. Native approval controls remain disabled until neutral-input/quiescence is established.

### W4. Complete mouse, keyboard and controller transitions

Wire the existing executor to actual authorized peer packets. Complete the feature ledger's mouse/key operations, keyboard/UI translations and any canonical extensions before advertising them. Enforce current source generation, per-action permits, sequence/rate bounds and local host priority. Virtual keys use exactly the same permission path.

The keyboard matrix includes key down/up/hold/repeat, modifiers, navigation/function/numpad keys, Ctrl+C/V/A/Z/S, Ctrl+Tab, Shift+Tab, permitted Alt+Tab and arrow combinations, layout/AltGr/dead-key behavior and bounded Unicode/text input where appropriate. Reconcile any absent wire kinds/shortcuts with the canonical contract. IME/security restrictions must be documented precisely; no secure attention sequence, unlock, UAC or protected-process bypass is supported.

Input suspension is terminal for old grants/permits. The current input_stop worker would repeatedly revoke newly admitted input grants; replace its generation/guard only after the old executor is fenced, reaped and released, with fresh grant/consent/lease. Preserve independent clipboard/file authority and the existing session registry. There is one exclusive native controller.

Verify fake sinks and actual process cleanup first. On approved disposable hardware, qualify emergency stop before cursor/key tests, then dragging, modifiers/layouts, hold/repeat, release-all, controller replacement, protected windows and recovery. No successful synthetic sink test substitutes for a physical action.

### W5. Complete clipboard and file data paths

Obtain the committed canonical resource handoff first. Implement native send/receive actions and role-specific authority, connecting the existing clipboard and session-owned file brokers. A publisher lease never authorizes the helper's local resources. Resource-only sessions must work without acquiring keyboard/mouse permission.

Text/image directions remain separate and default Off. Manual copy/paste and sync require explicit local/session permission, bounded formats, rate/size limits, loop prevention and immediate cleanup. Use reviewed decoding and encryption; do not log contents or promise automatic secret detection.

File capability consent never accepts an individual file. Use native local selection, recipient confirmation, safe destination handling, exact metadata/session/device/transfer binding, quotas, authenticated encrypted chunks, progress, hash verification and no-replace completion. Preserve Windows download marking/security scanning. Retry/resume requires freshly validated authority and immutable transfer/file binding. Drag/drop feeds the same pipeline; no raw path is exposed to the hosted page. No auto-execution/installation.

Intercept external file drops before the hosted page receives file objects. A remotely generated drag, including a cleanup-only mouse release, is not send consent. Show a separate trusted local per-file confirmation under the sensitive-UI guard before reading contents. Bind subsequent reads to the confirmed local file identities rather than a page-provided path.

Prefer an app-owned isolated/native drop target. Verify that the hosted renderer receives no file objects or bytes in any drag/drop phase before local confirmation; post-drop path filtering is too late. Incoming Electron drops otherwise use ordinary web file handling, as described in [Electron's documentation](https://www.electronjs.org/docs/latest/tutorial/native-file-drag-drop#dragging-files-into-your-app).

Test real selected/save dialogs and temporary files on approved devices, all role/direction combinations, image bombs/format limits, tamper/truncation, malicious names/reparse/collision, cancel while blocked, revocation at completion, quota retention and unauthorized resume. Media encryption alone is not data E2EE evidence.

### W6. Complete integrated host/guest experience and collaboration

Connect truthful native/web status, controller identity, capture/source controls, per-capability prompts, expiry and connection/error states. Provide trusted local stop/tray/shortcut controls effective with a frozen page/network, and accessible guest keyboard/shortcuts. Integrate existing pointer/annotation infrastructure; never grant OS input through visual overlay messages.

Add permissions/status management, clipboard actions, file accept/progress/cancel and visible drag targets. Source switching that broadens access requires a new host decision. New displays are never silently shared. Unknown capability/version combinations fail closed. Surface recoverable limitations rather than silently falling back.

Verify UI behavior and exact native side effects together. Preserve window controls, tray, notifications, settings, navigation, login, sandbox, session storage and signed updates.

### W7. Finish compatibility and network recovery

Consume exact committed web types/fixtures. Fix/reconcile the web client's eight-call safety-queue bypass through Claude. Use only approved STUN/TURN configuration and credential lifetimes; keep private certificates in their owning peers. Test real relay, loss, delayed packets, teardown, source removal, service/renderer restart and stale session/generation rejection. Never silently restore control, clipboard or transfer permission after revocation.

The peer's current 100ms queue bound measures residence after main receives IPC, not network/SCTP age. Resolve and test stale-input policy across transport interruption with the canonical contract before WAN acceptance. Separate media, input, collaboration and resource security evidence.

### W8. Qualify all Windows features on approved test devices

Prepare isolated test accounts and disposable service/media/data fixtures without production credentials. Native HTTPS keeps its fixed trusted origin and full certificate validation. If local native-to-service testing needs DNS/certificate fixtures, use a valid test certificate and explicit test-only issuer trust inside the approved disposable VM; do not alter the owner's host, production DNS/Nginx, production trust or TLS checking. Never ship fixture trust in a production artifact.

Use disposable Windows machines/accounts with explicit installation/native-test activation approval under Windows#5. Build a concrete test-only candidate first; no production feature is enabled by this plan. Record source SHAs, installer/native hashes, OS, display/DPI/layout, device roles, issuer/test authority and transport conditions. Test code must exercise the real packaged bridge, native coordinator and OS adapters, not a mock native endpoint.

Required sequence: native consent and independent emergency stop; then capture/input; clipboard/files; lifecycle and update cleanup; adversarial negatives; performance. Exercise real lock/user-switch/sleep/UAC/UIPI/secure desktop, process crashes and held-input release. Measure rather than invent latency/throughput/resource results. If hardware/approval is unavailable, record Not Verified and keep the readiness gate open.

### W9. Deliver the Windows integration candidate to web

Produce a fresh candidate version/tag distinct from the stopped v0.3.0 tag. Build all required native functionality in the qualified profile, verify source/ASAR/native bytes, fuses, identity/session preservation and exact-SHA High-risk checks. Production runtime gates remain controlled until separately approved.

Deliver exact Windows SHA, installer/native hashes, implemented bridge/protocol versions, exhaustive feature ledger, integration fixture runner, reproducible real-OS evidence, error/status semantics and remaining companion-only work. No known Windows implementation task in a required feature may remain open when declaring Windows integration-ready. Missing canonical resource/keyboard/annotation/resume contracts keep those features Blocked until reconciled and implemented.

### W10. Joint acceptance, approved release and follow-up

Run Windows-host/Windows-guest and Windows-host/web-guest matrices for every required capability. Test web-host/Windows-guest within the supported native-host model and web/web regression. A plain browser host has no native OS-control endpoint; keep that boundary truthful. Test all supported resource directions, E2EE/unauthorized observer negatives, stream end, original deadlines, group voice preservation, relay and recovery.

Run the actual installed upgrade/session-preservation procedure, including active Assist/pending transfers and failed cleanup. Produce the exact candidate evidence and practical guides. Obtain separate production enablement/release approval only after all required gates pass, then use the existing protected Ed25519-signing/GitHub Releases pipeline and verify published assets. Authenticode remains optional; TLS, Windows protections and updater signature checks remain enforced. Production issuer provisioning is project#47; private keys never enter source, artifacts or issues.

Update Windows#1/#3/#4/#5 and project#18/#33/#47 with exact evidence. Do not close incomplete items. Address window-size persistence after Assist readiness as already requested.

## Readiness rules

- **Windows component checkpoint:** reviewed component tests/build pass; no implication of full capability.
- **Windows integration-ready:** all required Windows feature implementations and real Windows acceptance pass, canonical contracts are reconciled, a packaged candidate and executable compatibility kit exist, and no Windows implementation work remains disguised as a web dependency.
- **Full Assist release-ready:** joint real-client, encryption/security, network/performance and installed-upgrade acceptance pass on exact candidate SHAs; issuer and release setup are qualified; separate activation/publication approval is recorded.

Current classification: component checkpoints Implemented/Experimental; Windows integration-ready Blocked; full release-ready Blocked. There is no date-only or test-count-only shortcut through these gates.

## Execution discipline

Implement W1-W4 while Claude finalizes the remaining canonical resource/relay handoff; prepare independent W5 components without inventing production APIs. Advance W5-W7 as exact contracts land, then W8-W10. At each security boundary: build, run focused positives/negatives, obtain independent review, run required checks/package evidence, commit a coherent slice on main and read its actual CI result. Tests and docs must distinguish fake sinks, real native processes, actual OS actions and joint service/media/data acceptance.

The canonical [integration guide](ASSIST-INTEGRATION.md), [native architecture](ASSIST-NATIVE-GUARDIAN.md), [threat model](ASSIST-THREAT-MODEL.md), [test evidence](TEST-RESULTS.md) and [companion handoff](ASSIST-COMPANION-HANDOFF-PROMPT.md) stay synchronized. No feature is accepted merely because its code is present or its gate remains disabled.
