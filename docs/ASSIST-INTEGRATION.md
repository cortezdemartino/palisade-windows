# Windows Assist integration

## Status and audited revisions

2026-10-24: **Blocked by the missing M4G-WEB contract.** Version 0.2.12 is an unpublished security-preparation candidate. It does not implement a desktop Assist bridge, source picker or screen-sharing host. Do not treat capability detection or a packaged build as Assist acceptance.

- Starting Windows SHA: a72055f8270eca79a17d0b64b3b85c768fe65748; 0.2.11, Electron 44.7.2.
- Audited committed web main: [231e96b83230834c7419779c311236b3330dc3bc](https://github.com/cortezdemartino/project-palisade/tree/231e96b83230834c7419779c311236b3330dc3bc). This is an audit reference, not a compatible integration revision.
- The local companion checkout was at 7da19ff0a5db0fd814ce8f73417d303b0d89759d and was not changed. Contract conclusions use committed GitHub files at the pinned main SHA.
- Windows enhancement: [#1](https://github.com/cortezdemartino/palisade-windows/issues/1). Missing handoff: [#2](https://github.com/cortezdemartino/palisade-windows/issues/2). Parent: [project-palisade#18](https://github.com/cortezdemartino/project-palisade/issues/18). Future native input review: [#33](https://github.com/cortezdemartino/project-palisade/issues/33).

## Contract audit

The full committed web tree has neither docs/45-ASSIST-CROSS-CLIENT-PROTOCOL.md nor docs/46-WINDOWS-ASSIST-BRIDGE-HANDOFF.md. Alternative desktop adapter/provider implementations and contract tests were also sought. None were found.

The actual [voiceApi.ts](https://github.com/cortezdemartino/project-palisade/blob/231e96b83230834c7419779c311236b3330dc3bc/apps/web/src/matrix/voice/voiceApi.ts) defines startScreenShare({ audio }) and stopScreenShare(). [sdkVoice.ts](https://github.com/cortezdemartino/project-palisade/blob/231e96b83230834c7419779c311236b3330dc3bc/apps/web/src/matrix/voice/sdkVoice.ts) calls LiveKit createLocalScreenTracks directly, then publishEncrypted, with screenRequest generation checks and track cleanup. These browser engine methods are not a desktop preload contract.

The [browser Assist foundation](https://github.com/cortezdemartino/project-palisade/blob/231e96b83230834c7419779c311236b3330dc3bc/docs/44-PALISADE-ASSIST.md) describes backend Assist protocol v1 and host/guest consent. **Backend protocol v1 does not define an Electron bridge version.**

| Required desktop contract | Audited state |
| --- | --- |
| Protocol ID/version and unsupported-version response | Missing |
| Global object, exported preload methods and capability schema | Missing |
| Screen-capture/viewer/endpoint/control/emergency-stop fields | Missing; no speculative fields exposed |
| Request/response schemas, errors, size/rate limits and timeouts | Missing |
| Status subscriptions, cancellation and listener cleanup | Missing |
| Current Assist generation/expiry, revocation and native stop handoff | Missing |
| Capture-provider ownership and late-publication prevention | Browser guards exist; native handoff missing |
| Executable shared contract tests and compatible web SHA | Missing |

The Windows integration must resume from the committed exports/tests when supplied. No future API names in planning documents authorize privileged IPC. Do not add a fallback globals interface or open capture merely because getDisplayMedia exists.

## Architecture and implemented security preparation

The existing restricted Electron architecture remains: frameless BrowserWindow; separate local controls WebContentsView; official hosted HTTPS view with sandbox, context isolation and no Node integration; persist:palisade-desktop partition; isolated one-way unread preload. Existing local controls IPC remains sender/main-frame/local-URL restricted. No Assist preload API or IPC event handlers have been added.

src/session-policy.cjs now owns the existing session permission registration, called by main with a getter for the current hosted contents. It checks exact WebContents identity, destruction/current navigation, Electron isMainFrame provenance and every supplied requesting/security/embedding origin. Same-origin subframes are denied. A stale contents reference loses authority as soon as the current getter changes.

Permission requests grant only notifications or a single audio media type to the trusted main frame. Permission checks grant media only for explicit audio, denying video and unknown types. Missing provenance and conflicting origin fields fail closed. Device permissions stay denied.

setDisplayMediaRequestHandler always returns null with useSystemPicker disabled. This is an explicit refusal path, not a capture implementation. No desktopCapturer import, source enumeration, preview, source identifier, reserved source, stream grant or native resource exists. The page receives only the standard browser rejection. There is no source metadata or capture telemetry.

API behavior was checked against installed electron.d.ts and [Electron 44.7.2 session documentation](https://github.com/electron/electron/blob/v44.7.2/docs/api/session.md). A null display response is the documented denial. The [desktopCapturer documentation](https://www.electronjs.org/docs/latest/api/desktop-capturer) is a future source-picker reference; its first-screen example must not be copied as consent behavior.

## Capture, consent and lifecycle

Native capture is **Blocked**, so no picker, native sharing indicator or host Stop Sharing action is claimed. Screen, window, screen audio, camera, clipboard/files and OS input remain unavailable through this preparation. The web page cannot enumerate native sources, authorize input or execute commands.

Pending/active native capture cancellation, display disconnect, lock/sign-out, sleep/resume, tray hiding, network uncertainty and session expiry must be designed against the actual web lifecycle contract before any source is granted. The current deny handler has no pending capture state to clean up. The existing Electron views clean up at window destruction, and permissions re-evaluate current contents/origin on each call.

The future native path must require explicit local source choice, no default or persistent grant, exact current Assist generation/expiry and fail-closed cancellation. It must connect local stop to actual track termination and late-publication prevention. Minimize/close-to-tray must not conceal ongoing sharing. No new tray capture semantics are activated now.

Backend participant consent, canonical session authorization, MatrixRTC/Rust key handling, LiveKit frame encryption and publication remain wholly in the existing web media engine. No Matrix token, recovery secret, media key, unencrypted stream, new endpoint or second media service is accessed here. Source review is not proof that E2EE activates in a Windows session.

## User workflow while integration is blocked

Use an approved, supported browser for Assist hosting, subject to the web installation being enabled and qualified. In the encrypted direct conversation, the host chooses Request assistance; the guest chooses Accept view-only assistance. Only then does the host choose Share screen and deliberately select a display or application window in the browser picker. Cancel leaves sharing off. Stop sharing releases the source; End assistance ends the session. Consult the web user guide at the audited revision for installation-specific availability.

The Windows candidate cannot host this flow. A denied desktop capture request is expected; do not bypass it with Chromium flags or broaden permissions. Windows viewing has not been qualified either. Remote keyboard/mouse control is not available in either this preparation or its API.

## Compatibility and acceptance matrix

| Scenario/check | Result |
| --- | --- |
| Windows host to web guest | Blocked; native capture not implemented |
| Web host to Windows guest | Not Verified |
| Windows host to Windows guest | Blocked / Not Verified |
| Web host to web guest regression | Not Verified in this Windows assignment; web source unchanged |
| Session identities, membership and live E2EE/unauthorized subscription | Not Verified across clients |
| Bridge version negotiation, capabilities, IPC limits/status/lifecycle | Blocked; no desktop protocol to test |
| Real installed picker, cancellation, refusal, multiple displays, window close, resolution/DPI | Not Verified |
| Physical stop/end/reload/shutdown/network/lock/sign-out/sleep | Not Verified for capture |
| Native endpoint/input | Not implemented; separately gated by #33 |

Windows 10/11 x64 remains the declared desktop target. This run does not establish capture support on either OS. The CI runner OS must be distinguished from an installed-client physical test.

## Verification and packaging

Run locked dependencies, npm run check, npm audit --omit=dev --audit-level=high and npm run package:win. Run npm run test:window-controls in source mode, then again with PALISADE_WINDOW_SMOKE_ASAR pointing to out/win-unpacked/resources/app.asar. This command includes existing controls/unread checks and the new session-policy-smoke.cjs fixture. Main-branch CI now runs the packaged suite for its exact SHA.

The permission fixture intercepts HTTPS with local synthetic HTML in an isolated test partition, supplies fake media devices without bypassing permission handlers, and checks microphone/camera/display/subframe/wrong-contents policy plus lack of privileged page globals. It neither loads the deployed web app nor accesses real screens, user accounts or physical microphones. No TLS check is disabled. Unit tests also cover null/missing provenance, contradictory origins, stale/destroyed contents, navigation, unrelated device permissions and explicit display refusal.

No new dependency, executable payload or production preload was added. Existing src/**/* packaging includes session-policy.cjs. Application ID, fuses, NSIS data preservation, browser partition, pinned key and manager dependency remain unchanged. Candidate builds are not official updates. No tag, release, installation or production activation is part of this checkpoint.

See TEST-RESULTS.md and M4G-WINDOWS-REPORT.md for executed evidence and limitations. Capture startup time, picker response, latency, CPU/memory/frame rate and native cleanup performance are **Not Verified**, not inferred from fixture timings.

## Recovery and next gate

If ordinary microphone/notification access regresses, inspect main-frame provenance and the fixed trusted origin against the Electron 44.7.2 handler contract. Do not allow subframes, unknown media or alternate origins to work around it. Keep existing user data; do not clear the encrypted Matrix profile.

The owner must supply the committed web contract through intervention #2. After review, implement the minimum compatible bridge, explicit local picker, contracted revocation and real encrypted cross-client tests. Use approved test devices/accounts privately. Native input needs independent architecture approval and remains outside view-only scope. Release/activation needs a later explicit owner decision after the required acceptance evidence exists.
