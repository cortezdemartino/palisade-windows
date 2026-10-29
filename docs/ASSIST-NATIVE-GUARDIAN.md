# Native Assist components

Status, October 28, 2026: **Experimental, inactive**. Production main/preload do not start or expose these components. The packaged helper has no sensitive Cargo features. This is not a full Assist test candidate.

The separate [private live channel](ASSIST-LIVE-CHANNEL.md) now implements typed framing, independent cancellation/liveness and application lifecycle ownership. Its negotiated cleanup scope is strictly channel-only: no runtime, input executor, identity or resource operation can start through it. Diagnostic v1 remains unchanged. Runtime integration and authenticated cleanup ownership through descendant crashes remain W2 work; the new channel cannot attest them.

## Process and authority

The Rust helper checks creator PID/creation time, packaged path, user SID, nonzero session, elevation and private pipes. Its original diagnostic protocol remains v1, with 16,384-byte frames and no generic command API. Diagnostic verification always returns authorized false.

The control-v2 verifier uses strict Ed25519 verification over canonical sorted-key UTF-8. It pins the deadline, distinguishes grant/lease generations, refuses revoked grants and binds permits to instance, action, epoch, source generation and a 100ms lifetime. Idle grant expiration invalidates held-input authority even under a fresh lease. Legacy v1 retains its fifteen-minute limit.

The coordinator joins local consent, endpoint-key-authenticated five-second polling and an independent input worker. WinHTTP keeps OS TLS/revocation checking at the fixed Palisade HTTPS origin, disables redirects/cookies/automatic authentication and bounds responses/timeouts. No Matrix credentials are extracted. Clock skew cannot extend a lease beyond twenty monotonic seconds from request start.

Consent displays helper account/device, capability and absolute expiry. Deny has default focus. Only the exact native Allow button notification approves. The worker revalidates the exact original grant, own revocation, source and current cancellation before recording consent. An unrelated input suspension cannot invalidate a separate file/clipboard decision through a shared epoch. Acknowledgement waits for the finished prompt worker before signing. A page callback cannot approve it.

Endpoint identity persists under current-user DPAPI at %LOCALAPPDATA%/Palisade/Assist/endpoint-v1.dpapi. Private identity material never crosses the pipe. Acknowledgements and lease requests sign the canonical purpose-labelled docs/49 statements. The raw enrollment-challenge IPC signer has been removed. Statement encoders alone provide no permission to sign or operate a device.

## Issuer trust and recovery

The native issuer loader accepts only an authenticated release fingerprint or a qualified local record. The dedicated Assist issuer is separate from the updater and Matrix keys. RELEASE_ISSUER_FINGERPRINT is deliberately unset until the production authority is established through [project-palisade#47](https://github.com/cortezdemartino/project-palisade/issues/47). A runtime HTTPS response cannot supply the release pin.

An administrator may instead compare the entire fingerprint shown by the trusted native confirmation window against the Server Controller's Assist page over a separate trusted channel. The default is Deny. The narrow prompt checks the ordinary desktop and cancellation, fetches the issuer again after confirmation, and refuses a changed value. This prompt is not exposed by diagnostic IPC or enabled by production main.

Qualified trust is stored in issuer-qualified-v1.dpapi as a strict versioned origin/key/fingerprint/method record under current-user DPAPI. The old issuer-v2.dpapi file is retained and never interpreted as confirmed trust. Existing qualified records are never overwritten; corruption, wrong origin, key changes and removed release pins fail closed. A completed administrator trust choice may remain if cancellation races its durable commit; the canceled caller receives no successful result. A live coordinator must recheck its operation/session epoch after this prompt and before enrollment or any acknowledgement.

For recovery, stop Assist first, revoke the affected endpoint enrollment and grants at the service, preserve the old record for review, and independently verify both old and new fingerprints. Use a reviewed release-pin migration or an explicitly reviewed administrator recovery procedure. There is no automatic reset, fallback to HTTPS or live key replacement. Never delete the endpoint identity to make a mismatch disappear. Re-enrollment and issuer recovery are not yet wired into the application UI, and their physical acceptance remains Not Verified.

## Application exit and update ownership

The dormant runtime now provides a prepared RuntimeRevoker whose revoke/suspend methods touch only atomic flags. Global stop, input-only stop/readiness, poller cancellation and resource closure are independent of registry/UI/I/O/join locks. Native prompt contexts observe the same global stop and, for input prompts only, the input-stop generation. Cancel is set before a prompt cleanup attempts its gate lock.

Blocking stop is a separate retained operation. It joins prompt, polling, input and file cleanup without treating a consumed handle as success. Failed owners stay retained and failed results remain failed on retry. A self-join is rejected before acquiring the join mutex, while an external owner retains the real handle. Cleanup/destructor errors cannot suppress later independent cleanup attempts. These are internal APIs; the channel-only protocol does not expose a runtime stop/completion proof.

The helper release profile uses Rust unwinding so caught worker/cleanup panics can finish this path. The fixed parent-validated cleanup probe tests the actual release executable, including a panic payload whose destructor also panics, sticky failure and a later worker join. It has no identity, UI, input, resource or general request access. Cargo tests alone cannot establish the release panic strategy because [the test harness ignores that profile setting](https://doc.rust-lang.org/cargo/reference/profiles.html#panic). Recovery covers unwinding Rust panics; [abort and foreign/FFI failures are outside catch_unwind's guarantee](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html). They still require crash-surviving descendant ownership and failed lifecycle evidence, not a cleanup-success claim.

SessionFiles.close now marks cancellation without waiting for a registry/gate/store lock or sweeping I/O. Its watcher performs cleanup. Explicit wait_cleanup waits for selecting operations, both store shards and all owned transfer handles before joining that watcher. Poisoned state is recovered only to cancel/release and reports failed cleanup; an empty operation table alone is insufficient. Dropping a broker or seeing a finished flag is not lifecycle completion evidence.

The application now shares one trusted-main lifecycle barrier between Quit and the update manager. Holders register before admitting authority, revoke synchronously and supply Promise<void> only after their owned resources and cleanup children have exited. All holders are revoked before any cleanup wait; new holders are refused after shutdown starts. A failed wait retains ownership and allows a proof-only retry without reopening authority. Normal holder completion also retains its record until cleanup is confirmed.

The manager requires this preparation hook before creating/copying/spawning its executable. Stop during preparation or copying prevents launch. Update polling stops on will-quit, so a prevented quit does not silently disable automatic checks. The quit handler keeps the window alive until preparation and profile flush complete, coalesces repeated requests, reports pending cleanup and retries failures. No timeout kills a holder. A failed exit attempt requires completing exit/relaunch before new Assist authority can be admitted by this lifecycle instance.

This is application wiring, not proof of native child cleanup: the current holder covers existing capture behavior, and the live coordinator must register the actual endpoint/release holders. The real Electron quit fixture uses a fake holder and confirms failed/pending cleanup keeps the window/process open, progress is reported, profile work waits, and retry exits only after completion. Actual native/OS/installed-upgrade acceptance remains required.

The direct guardian's exit must not resolve the holder if an owned release-only child survives. The live broker needs authenticated evidence that every owned cleanup holder has completed and exited, including crash paths. Process-name polling or elapsed time does not supply that proof.

## Owned remote input transport

The inactive trusted-main peer uses an app-owned sandboxed window and isolated preload. It has no hosted-page API, Node integration or permission grants. Its factory defaults disabled and production does not call it. The coordinator must supply a session and helper fingerprint from a grant independently verified by the native runtime. The transport itself cannot authorize input.

The offer and answer contain exactly one SCTP application transport and one SHA-256 fingerprint; audio, video, duplicate fingerprints, extra media sections and unsupported attributes are refused. The offer must match the approved helper certificate. Before any packet reaches its native callback, the peer hashes the actual remote DTLS certificate, compares it with that fingerprint and waits for native peer acceptance. Pre-acceptance packets are bounded and discarded. The answer and its own fingerprint are internal results, not signed authority. Wiring the native transport-statement signature and canonical peer operations remains coordinator work; no generic signer has been introduced.

Only one ordered, reliable `control` channel with the expected protocol is accepted. Extra/wrong channels, binary messages, packets over 2,048 UTF-8 bytes, rate/queue overflow and packets queued in main for at least 100ms fail closed. This queue limit does not measure network/SCTP retransmission age or establish sender-to-host freshness; WAN recovery remains unqualified. The original packet bytes reach the native strict decoder, which independently checks exact grant, generation, capability, replay, action and current authorization. This component does not replace those checks. Native acceptance and packet callbacks receive an abort signal; the live coordinator must also recheck its native session flags during dispatch.

Private nonce/document-scoped IPC, fresh monotonic heartbeats, renderer/navigation/connection loss and external cancellation close the transport. Closing invokes the coordinator's revocation callback synchronously before destroying the peer. The returned `finished` promise retains any asynchronous cleanup result. It does not itself establish input-child exit; the future coordinator and quit/update path must retain and await native owned-input cleanup. A callback failure must postpone installation, never justify killing the last cleanup holder.

No page-provided ICE server or credentials are accepted. The current peer has an empty local ICE-server configuration and validates bounded incoming ICE, including authenticated remote relay candidates. Approved native STUN/TURN configuration and actual WAN/TURN qualification remain dependencies. Unlike the local capture hop, this peer is intended to connect to a remote helper; local socket-route restrictions are not reused as remote identity proof. Input DTLS is not evidence for clipboard/file E2EE or for cross-client media encryption.

`npm run test:assist-peer` runs real Electron 44.7.2 peers with a fake native sink: actual certificate checks and substitution failure, early-packet discard, private-world/provenance checks, delayed cleanup, canceled admission/dispatch, wrong/extra channels, binary/oversized traffic, stalled queues, crashes and frozen renderers. No OS input, native enrollment or service session is exercised. Repeat with `PALISADE_WINDOW_SMOKE_ASAR` pointing to the built archive for packaged evidence. Both build and release workflows require this check before artifact publication/signing.

## Input executor

Native enrollment now has a private, non-deserializable in-memory tuple binding the qualified fixed-origin issuer, installation public key, account, device and endpoint ID. The trusted enrollment window displays those values and defaults to Deny. After confirmation, the code checks cancellation, the ordinary desktop and the qualified issuer again, then signs only the exact labelled proof. The tuple is not proof of service registration. Restart/re-enrollment requires new local confirmation; no permanent remote permission is saved.

Native session construction now requires that tuple and the matching identity. Publisher/account/device/endpoint expectations come from it; room and the tentative track SID come from the owned publication; helper details must verify under the signed grant. The tuple registers bounded session-stop flags. Revocation or Drop sets every old flag, including during concurrent admission, and refuses old proofs/bindings. A replacement needs fresh local approval and a fresh stop owner; old stop flags are never reset. No enrollment proof route has been restored to diagnostic IPC.

Enrollment and capability consent are separate. A pending signed grant may receive its native consent acknowledgement before the service activates it. The acknowledgement alone permits no action: a fresh independently fetched covering lease and the observed approved DTLS peer are still mandatory. Input/resource execution, real native dialog interaction and service enrollment remain unqualified.

The guardian owns the authorization gate and a write-ahead ledger. A child accepts bounded, sequenced, short-lived primitive steps over creator-bound inherited pipes. It has no listener, command launcher or elevated service. Before self-spawn, the helper checks the protected expected binary hash and retains no-write/no-delete image/ancestor locks.

A fixed per-session named-event ownership object is retained by both guardian and child. Every existing object blocks a successor, regardless of signaling. The child duplicates the exact object from its validated parent and checks kernel-object identity before readiness. The guardian retains ownership through fenced mirror cleanup, including unwind. A two-process fixture kills the initial holder and proves repeated attempts remain blocked until the surviving holder exits. This proves lifetime exclusion, not physical held-key recovery.

The child has another local ledger, a local activity monitor and independent parent/idle watchdog. Heartbeats contain no input. Shutdown stops steps before cleanup. Guardian cleanup fences/reaps the child before mirrored UP events. Owner Drop retains native cleanup on failures. If its guardian dies, the child retains only owned release authority, refuses protected-desktop injection and retries when Default returns. Simultaneous process/OS failure is not claimed to be recoverable.

The adapter validates physical monitor bounds, source generation, virtual-desktop coordinates, per-monitor DPI, session, target SID and elevation. It denies OS/Meta keys, Alt+Tab, secure-attention/elevation shortcuts, unknown scan codes and unowned UP events. Pre-existing local mouse holds cannot become remote drags. Physical layouts, AltGr/dead keys/IME, modifier interference, crash recovery and protected-window transitions remain unqualified. Input failure/suspension terminally revokes current mouse/keyboard grants and releases owned input while independently authorized resources keep their checks. A full service renewal cannot restore locally revoked input. Clock, lease, desktop/session uncertainty and Stop Assist still terminate the whole session.

Probe mode simulates steps/cleanup. It does not register activity hooks, enroll identities or execute resource/input APIs, including in all-features builds. Actual subprocess checks cover readiness, no-input keepalive, EOF, idle loss, malformed frames, expiry and replay. These do not qualify physical stop shortcuts or real input.

Both global session stop and input-only stop are checked at each dispatch clock barrier and before primitive application, including after a blocking intent acknowledgement. A stop raised during the final step also returns through fenced owned-state cleanup. Fake-backend tests cover stop during intent, between shortcut steps and during a single DOWN. Separate resource revocation still cannot invalidate unrelated input permits, and input suspension does not grant or silently revoke independent resource authority.

## Clipboard and files

The trusted-main clipboard broker has publisher/helper directions, positive authority checks, directional cancellation, default Off and a local copy action. Sync requires a negotiated agreement on every operation. Epoch checks stop paused in-flight work. It stores one bounded incoming text item without history or content logging. OS/peer integration is pending.

The runtime now owns a publisher-side SessionFiles adapter. A native FileRequest retains immutable session instance, exact grant/generation, direction, transfer ID and selected/offered metadata. Requests are reserved before UI; they are not deserialized from the hosted page. A receive grant is separate from the native confirmation of one file's helper/device, name, size and digest before its save dialog. File UI remains unqualified; tests simulate that decision.

Two directional stores use separate locks and one atomic non-refunding session quota, so a send dialog/hash does not block receiving. A cancellation watcher checks current native authority and expires idle/pending work without another packet. It invalidates flags before trying cleanup locks, rejects late selection and token/session swaps, and preserves the other direction/input. Runtime callers obtain an Arc without holding the runtime during dialogs. Close immediately cancels work; a cleanup-only watcher can retain owned handles until blocked OS I/O unwinds. The underlying callback-taking FileStore is private to this module.

Native file selection exposes opaque handles. It accepts ordinary files on fixed NTFS volumes, checks/locks each ancestor without delete sharing and resolves subsequent opens from volume-GUID handles. Sources cannot change while held. Limits are 1GiB/file, one transfer/direction and 4GiB accepted bytes/store-session; canceled attempts remain charged. Operation cancellation closes dialogs and refuses late opens.

Receivers use unpredictable exclusive private staging, current-user/SYSTEM ACLs and delete-on-close. Zone.Identifier contains ZoneId=3. Size/SHA-256 must match before atomic handle rename with no replacement. Delete-on-close remains through marking/rename and clears only on successful commit. Failed/canceled operations delete only their owned handle. Temporary NTFS tests cover immutability, tampering, collisions, cancellation, quotas, parent rename refusal, zero bytes, download marking and OS staging cleanup after child exit without Rust Drop. No personal files or clipboard were used.

## Reproducible checks

On Windows x64 with the pinned toolchain and Visual Studio linker:

~~~powershell
npm ci
npm run check
npm run test:assist-native
npm run build:assist
npm run test:assist-process
cargo audit --file native/assist/Cargo.lock --deny warnings
npm run package:win
npm run verify:assist-package
$env:PALISADE_WINDOW_SMOKE_ASAR = (Resolve-Path out/win-unpacked/resources/app.asar).Path
npm run test:window-controls
npm run test:assist-capture
Remove-Item Env:PALISADE_WINDOW_SMOKE_ASAR
~~~

Native tests compile both feature configurations, exercising fake sinks, owned hidden windows, temporary identities and fixture files. Process tests repeat non-authorizing probes against an all-features debug binary, which is not packaged. Packaging rebuilds the locked default helper, includes licenses and embeds hash/size in the integrity-protected ASAR. No runtime download or private release key is included.

## Integration gates

Complete the owned DTLS gateway, capture provenance and versioned page endpoint contract before exposing operations. Reuse one native stop owner. The live watchdog must invalidate/fence and retain cleanup rather than use diagnostic immediate exit. Complete canonical source-end wiring. Native controller exclusion and input/resource separation are implemented internally; qualify their live application integration. Suspended input requires fresh authorization and local approval, never an old grant or renewed lease. Qualify consent, stop, input, lock/sleep, DPI, encryption and recovery on disposable Windows devices. Installation, activation and release need separate owner approval.
