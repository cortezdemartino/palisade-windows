# Native Assist components

Status, October 26, 2026: **Experimental, inactive**. Production main/preload do not start or expose these components. The packaged helper has no sensitive Cargo features. This is not a full Assist test candidate.

## Process and authority

The Rust helper checks creator PID/creation time, packaged path, user SID, nonzero session, elevation and private pipes. Its original diagnostic protocol remains v1, with 16,384-byte frames and no generic command API. Diagnostic verification always returns authorized false.

The control-v2 verifier uses strict Ed25519 verification over canonical sorted-key UTF-8. It pins the deadline, distinguishes grant/lease generations, refuses revoked grants and binds permits to instance, action, epoch, source generation and a 100ms lifetime. Idle grant expiration invalidates held-input authority even under a fresh lease. Legacy v1 retains its fifteen-minute limit.

The coordinator joins local consent, endpoint-key-authenticated five-second polling and an independent input worker. WinHTTP keeps OS TLS/revocation checking at the fixed Palisade HTTPS origin, disables redirects/cookies/automatic authentication and bounds responses/timeouts. No Matrix credentials are extracted. Clock skew cannot extend a lease beyond twenty monotonic seconds from request start.

Consent displays helper account/device, capability and absolute expiry. Deny has default focus. Only the exact native Allow button notification approves. The worker revalidates the grant, source and epoch before recording consent. Acknowledgement signs the native grant after that decision. A page callback cannot approve it.

Endpoint identity persists under current-user DPAPI at %LOCALAPPDATA%/Palisade/Assist/endpoint-v1.dpapi. The issuer-v2 pin implementation confirms the fetched fingerprint on first use and refuses changes. This is TOFU with HTTPS, not an independently authenticated release pin. Production enrollment still requires reviewed trust policy. Never automatically delete corrupt identities or changed pins. Recovery must revoke old service enrollment/grants before establishing new authority. Private identity material never crosses the pipe.

## Input executor

The guardian owns the authorization gate and a write-ahead ledger. A child accepts bounded, sequenced, short-lived primitive steps over creator-bound inherited pipes. It has no listener, command launcher or elevated service. Before self-spawn, the helper checks the protected expected binary hash and retains no-write/no-delete image/ancestor locks.

The child has another local ledger, a local activity monitor and independent parent/idle watchdog. Heartbeats contain no input. Shutdown stops steps before cleanup. Guardian cleanup fences/reaps the child before mirrored UP events. Owner Drop retains native cleanup on failures. If its guardian dies, the child retains only owned release authority, refuses protected-desktop injection and retries when Default returns. Simultaneous process/OS failure is not claimed to be recoverable.

The adapter validates physical monitor bounds, source generation, virtual-desktop coordinates, per-monitor DPI, session, target SID and elevation. It denies OS/Meta keys, Alt+Tab, secure-attention/elevation shortcuts, unknown scan codes and unowned UP events. Pre-existing local mouse holds cannot become remote drags. Physical layouts, AltGr/dead keys/IME, modifier interference, crash recovery and protected-window transitions remain unqualified. This checkpoint conservatively ends the session on child/activity loss; resource continuity during input suspension remains integration work.

Probe mode simulates steps/cleanup. It does not register activity hooks, enroll identities or execute resource/input APIs, including in all-features builds. Actual subprocess checks cover readiness, no-input keepalive, EOF, idle loss, malformed frames, expiry and replay. These do not qualify physical stop shortcuts or real input.

## Clipboard and files

The trusted-main clipboard broker has publisher/helper directions, positive authority checks, directional cancellation, default Off and a local copy action. Sync requires a negotiated agreement on every operation. Epoch checks stop paused in-flight work. It stores one bounded incoming text item without history or content logging. OS/peer integration is pending.

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
Remove-Item Env:PALISADE_WINDOW_SMOKE_ASAR
~~~

Native tests compile both feature configurations, exercising fake sinks, owned hidden windows, temporary identities and fixture files. Process tests repeat non-authorizing probes against an all-features debug binary, which is not packaged. Packaging rebuilds the locked default helper, includes licenses and embeds hash/size in the integrity-protected ASAR. No runtime download or private release key is included.

## Integration gates

Complete the owned DTLS gateway, capture provenance and versioned page endpoint contract before exposing operations. Reuse one native stop owner. The live watchdog must invalidate/fence and retain cleanup rather than use diagnostic immediate exit. Enforce one controller across streams and source-end teardown. Preserve resources during input suspension and require fresh approval to resume control. Qualify consent, stop, input, lock/sleep, DPI, encryption and recovery on disposable Windows devices. Installation, activation and release need separate owner approval.
