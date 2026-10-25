# Windows Assist integration

## Current M4G-WINDOWS-FULL checkpoint

**Desktop version 0.3.0. Overall Assist product: Blocked.** The desktop release retains disabled Assist capture/input/data routes; publication does not activate or qualify them. Starting Windows SHA: ef5dab42da44f5f31d2bfe2530f9f6ee2365d36c. The committed web v1 contract is now available at **6c7ab894862ca3767b1239594e37c322e96ff2c4**. The earlier missing-contract audit below is historical, not current status.

### Implemented canonical v1 bridge

The hosted main frame exposes exactly window.palisadeDesktopAssist with protocol, getCapabilities(request), and onCapabilitiesChanged(listener). Protocol is palisade.assist.bridge, version 1. Main uses the exact handoff channels palisade:assist-capabilities and palisade:assist-capabilities-changed. Only a bounded integer version list reaches main; no tokens, Matrix identities, session IDs, paths or native commands cross this API.

The seven report fields are protocol, version, desktopVersion, capture, nativeEndpoint, localRevocation and emergencyStop. This candidate reports capture unavailable, nativeEndpoint absent, localRevocation false and emergencyStop false. The 0.3.0 desktop version meets the web minimum. No other control/data operation is added to v1.

Main validates exact hosted WebContents, current trusted main frame and URL, payload keys/version bounds, 12 requests/second and navigation state. The isolated preload bounds requests/subscriptions, has a 2.5-second timeout inside the web adapter 3-second deadline, strips Electron events and removes listeners on unload. Main removes its handler at contents destruction. The pinned actual web TypeScript adapter is retained as a test fixture with a SHA-256 assertion; tests execute its parser/adapter against our reports. Node 22.13+ is required for this test compiler API; CI uses the current Node 22 release.

### Capture components and security blocker

Implemented components include the getDisplayMedia controller, explicit local screen/window picker, thumbnails, keyboard radio selection, Share/Cancel, source revalidation, one pending request, no automatic selection, video only, cancellation/timeout/generation guards and a 15-minute local upper bound. The controller rejects background/minimized windows, wrong frame/origin, absent gesture, audio requests and concurrent sharing. It re-enumerates the chosen source before granting. Source names/previews stay in the trusted local picker; titles are rendered as text under a fixed CSP. No source list is exposed to the hosted page.

**Production capture is disabled in src/assist/build-policy.cjs. Do not toggle that flag alone.** An actual Windows Electron 44.7.2 display request arrived as media with mediaTypes: [] before the display handler. Its public RequestMediaAccessPermission implementation uses the same empty shape for modern display and legacy desktop/tab capture, and MediaAccessAllowed routes legacy capture outside ChooseDisplayMediaDevice. Allowing empty media requests would bypass picker enforcement. The unchanged microphone-only rule refuses them. A safe runtime/API architecture must be independently reviewed in [Windows #3](https://github.com/cortezdemartino/palisade-windows/issues/3).

Reference: [pinned Electron permission implementation](https://github.com/electron/electron/blob/v44.7.2/shell/browser/web_contents_permission_helper.cc). The failed positive capture experiment is retained as a finding, not reported as capture success. The real Electron suite proves production modern/legacy denial and the separate local picker UI. It does not establish native capture, physical mouse routing, installed-client sharing or E2EE.

The follow-up admission fixture tests fifteen request shapes with both an absent and a nominally available controller. Electron 45.0.0 distinguishes display requests, but its generic display grant still covers legacy capture outside the picker. Before the guard, the available-controller fixture observed four unexpected permission grants on that runtime; the fixture refused them before capture. Session policy now denies all display paths independently of availability, and main reports capture unavailable independently of the controller flag. Both runtimes pass the thirty-case negative matrix after that change. Mixed legacy video/microphone combinations terminated their synthetic renderers before the permission callback; this is recorded separately from normal permission denial and does not confirm a capture exploit. Electron remains pinned to 44.7.2.

The [resolution plan](ASSIST-RESOLUTION-PLAN.md) sets the next order: enforceable capture admission and encrypted viewing, independent local stop, scoped input, then independently approved clipboard/files. Its invitation/access-pass defaults are proposed shared authorization behavior, not a new v1 API or an implemented authority.

Local stop is wired to a tray action, minimize/hide/close/unresponsive, renderer navigation/crash, Windows lock/suspend/shutdown and display changes. If a source had been granted, the conservative mechanism terminates the renderer and reloads Palisade; it does not rely on page cooperation and ends calls as well. The action is inactive when no grant/picker exists. Renderer termination was tested; active-capture termination and the OS lifecycle matrix remain Not Verified. Booleans remain false. Canonical web session expiry/revocation still owns its tracks; v1 gives native no session-scoped cancellation event, so immediate native dismissal of a picker after remote session end requires further contract work. Existing web late-track guards must never be bypassed.

### Independently implemented non-injecting components

- lease.cjs: mandatory independent-verifier seam, exact participant/device/endpoint/session binding, independently supplied original session deadline, bounded capability subset, separate live in-memory consent, 15-minute maximum, expiry timer, replay/generation/rate/clock checks, scoped cancellation and terminal revocation. Revoked scopes cannot be restored on the same lease.
- input-model.cjs: content/source rectangle translation with negative origins and rotation, bounded key/button plans, reviewed-shortcut proposal and release-all ledger. No SendInput, keyboard hook, Unicode injector or virtual-keyboard UI exists.
- data-model.cjs: bounded UTF-8/PNG envelope handling, opt-in in-memory deduplication, Windows basename/size/hash validation and bounded writable-stream reception with pre-read authorization, scoped cancellation, failed-destination interruption and final integrity/lifecycle checks. No OS clipboard operation, filesystem path, file dialog/save broker, encryption or network transfer exists.

These helpers are not imported by any production operation handler. Tests use synthetic independent authority, in-memory streams and planner outputs; they do not establish endpoint identity or native execution. Host-owned input/clipboard/file consent UI and trusted verifier/transport remain Blocked. Annotations/pointers must reuse the actual existing Olm to-device annotation implementation, after the service grants Assist collaboration.

### Product and acceptance status

| Capability | Classification |
| --- | --- |
| v1 description API and pinned web parser compatibility | Implemented; actual Electron fixture passed |
| Trusted picker and capture controller components | Implemented / Experimental; production gate off |
| Secure native screen/window capture and all encrypted cross-client sharing | Blocked / Not Verified |
| Native endpoint, OS mouse/keyboard/shortcuts/virtual keyboard, real emergency stop | Blocked on independent approval project-palisade#33 |
| Clipboard/files/annotations and encrypted data transport | Helpers implemented; end-to-end Blocked on Windows #4 |
| Mixed-DPI monitors, secure desktop, actual input/lock/clipboard/file acceptance, WAN/performance | Not Verified |

The full user workflow requires separately consented viewing, control, clipboard directions and file send/receive. No functional unsupported buttons or hidden native operations are exposed now. Use the existing approved browser view-only flow where enabled; do not attempt to enable input or clipboard through v1 or bypass desktop capture denial. On disconnection, future sensitive channels must terminate and require fresh proof/consent, never silently resume.

Current scope, proposed types/examples/states and companion integration points: [ASSIST-CONTROL-HANDOFF.md](ASSIST-CONTROL-HANDOFF.md). Native process/OS/consent decision: [ASSIST-ENDPOINT-REVIEW.md](ASSIST-ENDPOINT-REVIEW.md). Threats/evidence/risks: [ASSIST-THREAT-MODEL.md](ASSIST-THREAT-MODEL.md). Full results and known limitations: [M4G-WINDOWS-REPORT.md](M4G-WINDOWS-REPORT.md) and [TEST-RESULTS.md](TEST-RESULTS.md).

Windows 10/11 x64 remains the declared target; qualification across those platforms is not claimed. Application identity, persist:palisade-desktop, signed-update trust, manager pin and security fuses remain unchanged. src/**/* packages the new local picker/preloads/modules; fixtures/tests are not shipped. No new dependency or native executable is added. A build artifact is not approval to install or activate.

## Earlier 0.2.12 checkpoint: retained audit history

The following dated material described the previous web revision before the v1 handoff was committed. The current status above supersedes its missing-v1-contract statements.


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
