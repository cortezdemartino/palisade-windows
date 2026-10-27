# Native Assist components

Status, October 26, 2026: **Experimental, inactive**. Production main/preload do not start or expose these components. The packaged helper has no sensitive Cargo features. This is not a full Assist test candidate.

## Process and authority

The Rust helper checks creator PID/creation time, packaged path, user SID, nonzero session, elevation and private pipes. Its original diagnostic protocol remains v1, with 16,384-byte frames and no generic command API. Diagnostic verification always returns authorized false.

The control-v2 verifier uses strict Ed25519 verification over canonical sorted-key UTF-8. It pins the deadline, distinguishes grant/lease generations, refuses revoked grants and binds permits to instance, action, epoch, source generation and a 100ms lifetime. Idle grant expiration invalidates held-input authority even under a fresh lease. Legacy v1 retains its fifteen-minute limit.

The coordinator joins local consent, endpoint-key-authenticated five-second polling and an independent input worker. WinHTTP keeps OS TLS/revocation checking at the fixed Palisade HTTPS origin, disables redirects/cookies/automatic authentication and bounds responses/timeouts. No Matrix credentials are extracted. Clock skew cannot extend a lease beyond twenty monotonic seconds from request start.

Consent displays helper account/device, capability and absolute expiry. Deny has default focus. Only the exact native Allow button notification approves. The worker revalidates the grant, source and epoch before recording consent. Acknowledgement signs the native grant after that decision. A page callback cannot approve it.

Endpoint identity persists under current-user DPAPI at %LOCALAPPDATA%/Palisade/Assist/endpoint-v1.dpapi. Private identity material never crosses the pipe. Acknowledgements and lease requests sign the canonical purpose-labelled docs/49 statements. The raw enrollment-challenge IPC signer has been removed. Statement encoders alone provide no permission to sign or operate a device.

## Issuer trust and recovery

The native issuer loader accepts only an authenticated release fingerprint or a qualified local record. The dedicated Assist issuer is separate from the updater and Matrix keys. RELEASE_ISSUER_FINGERPRINT is deliberately unset until the production authority is established through [project-palisade#47](https://github.com/cortezdemartino/project-palisade/issues/47). A runtime HTTPS response cannot supply the release pin.

An administrator may instead compare the entire fingerprint shown by the trusted native confirmation window against the Server Controller's Assist page over a separate trusted channel. The default is Deny. The narrow prompt checks the ordinary desktop and cancellation, fetches the issuer again after confirmation, and refuses a changed value. This prompt is not exposed by diagnostic IPC or enabled by production main.

Qualified trust is stored in issuer-qualified-v1.dpapi as a strict versioned origin/key/fingerprint/method record under current-user DPAPI. The old issuer-v2.dpapi file is retained and never interpreted as confirmed trust. Existing qualified records are never overwritten; corruption, wrong origin, key changes and removed release pins fail closed. A completed administrator trust choice may remain if cancellation races its durable commit; the canceled caller receives no successful result. A live coordinator must recheck its operation/session epoch after this prompt and before enrollment or any acknowledgement.

For recovery, stop Assist first, revoke the affected endpoint enrollment and grants at the service, preserve the old record for review, and independently verify both old and new fingerprints. Use a reviewed release-pin migration or an explicitly reviewed administrator recovery procedure. There is no automatic reset, fallback to HTTPS or live key replacement. Never delete the endpoint identity to make a mismatch disappear. Re-enrollment and issuer recovery are not yet wired into the application UI, and their physical acceptance remains Not Verified.

## Input executor

The guardian owns the authorization gate and a write-ahead ledger. A child accepts bounded, sequenced, short-lived primitive steps over creator-bound inherited pipes. It has no listener, command launcher or elevated service. Before self-spawn, the helper checks the protected expected binary hash and retains no-write/no-delete image/ancestor locks.

A fixed per-session named-event ownership object is retained by both guardian and child. Every existing object blocks a successor, regardless of signaling. The child duplicates the exact object from its validated parent and checks kernel-object identity before readiness. The guardian retains ownership through fenced mirror cleanup, including unwind. A two-process fixture kills the initial holder and proves repeated attempts remain blocked until the surviving holder exits. This proves lifetime exclusion, not physical held-key recovery.

The child has another local ledger, a local activity monitor and independent parent/idle watchdog. Heartbeats contain no input. Shutdown stops steps before cleanup. Guardian cleanup fences/reaps the child before mirrored UP events. Owner Drop retains native cleanup on failures. If its guardian dies, the child retains only owned release authority, refuses protected-desktop injection and retries when Default returns. Simultaneous process/OS failure is not claimed to be recoverable.

The adapter validates physical monitor bounds, source generation, virtual-desktop coordinates, per-monitor DPI, session, target SID and elevation. It denies OS/Meta keys, Alt+Tab, secure-attention/elevation shortcuts, unknown scan codes and unowned UP events. Pre-existing local mouse holds cannot become remote drags. Physical layouts, AltGr/dead keys/IME, modifier interference, crash recovery and protected-window transitions remain unqualified. Input failure/suspension terminally revokes current mouse/keyboard grants and releases owned input while independently authorized resources keep their checks. A full service renewal cannot restore locally revoked input. Clock, lease, desktop/session uncertainty and Stop Assist still terminate the whole session.

Probe mode simulates steps/cleanup. It does not register activity hooks, enroll identities or execute resource/input APIs, including in all-features builds. Actual subprocess checks cover readiness, no-input keepalive, EOF, idle loss, malformed frames, expiry and replay. These do not qualify physical stop shortcuts or real input.

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
