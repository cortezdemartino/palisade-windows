# Validation

## Private live channel checkpoint, October 28, 2026

Starting Windows SHA: `7afbc26be921f12ec2b722b4948a2025d7434453`; its exact-SHA Windows CI run 40610053435 passed. Companion refreshed read-only at `82ac7de4ec2bbd5cc2fd48f3d15dbde6ea5fd375`; docs/50 now supplies the initial text/file/relay/cleanup extension. No companion acceptance was executed by the Windows agent.

- `npm run check`: 166 application tests and configured syntax checks passed. Nine new live-channel groups cover lifecycle-before-spawn, cancellation during verification, failed launches, exact cleanup scope/nonce plus actual exit, full ordinary capacity with reserved shutdown, late results, fragmented/coalesced responses, malformed/duplicate/UTF-8/oversized output, backpressure and external abort.
- Rust format/strict Clippy passed. The native suite passed 72 tests in each default/all-features configuration: 49 library, 12 executor, four input-model and seven stream-authority tests. A first run exposed Serde's internally tagged unit-variant acceptance of extra fields; explicit empty struct command variants fixed it, and the suite passed afterward. Diagnostic framing remains 16,384 bytes; only the separate private protocol uses 65,536.
- Sixteen actual Windows subprocess groups passed in each configuration, plus the hidden-startup visibility fixture. New live probes repeat adjacent handshake/shutdown, interrupt eight stalled calls, maintain pulses during a 5.5-second ordinary wait, refuse malformed/replayed/partial/unsafe frames, bound queue/output pressure, detect parent death and retain failed lifecycle ownership after an unproven probe crash. No probe injects input, opens an identity/clipboard or starts a native session.
- Fresh Windows x64 NSIS packaging and native integrity passed. Packaged desktop/permissions/admission/quit checks, all five capture groups and all 14 actual-DTLS/fake-sink peer cases passed. The new channel and existing main ASAR bytes match source. The sixteen Chromium bad-IPC terminations are intentional admission negatives. Locked native and production npm audits found zero vulnerabilities.
- Independent read-only review cleared the inactive channel-only slice after the shutdown metadata-publication race was fixed and independently passed the initial eight JavaScript groups. A ninth launch-failure group was subsequently added and passed locally. Native/runtime cleanup panics and crash-surviving descendants remain pre-authority integration requirements, not qualified by these tests.

Unsigned local installer SHA-256: `23034c283f58ebd2b173146f23820c578b9b7b0eece6b5431f8b7a67de52acf3`. The new push needs its own exact-SHA Windows CI result, recorded in Windows#1 after execution. No installation, signing, native activation or release publication occurred. Full Assist remains Blocked: this channel cannot start the runtime or attest active input/resource/descendant cleanup. Physical Windows, encrypted joint sessions, working TURN, performance and installed-upgrade acceptance remain Not Verified.

## Full feature plan and application cleanup barrier, October 27, 2026

Starting Windows SHA: `7fcae472c041b1569233880cff96ae0a691bc7d1`, whose exact-SHA Windows CI run 40602184816 passed. Companion refreshed read-only at `ae89e414ec71a193094ef7cbee5ad678418ff6f6`. Risk: High, application exit/update and cleanup ownership. The complete feature ledger and W1-W10 plan keep all requested capabilities open until implemented and qualified.

- `npm run check`: 157 application tests and configured syntax checks passed. Nine lifecycle groups and five added updater groups cover immediate/coalesced revocation, all-holder cleanup retention, reentrancy, failed/proof-only retry, invalid completion contracts, delayed status, profile flush, manager preparation before filesystem/spawn and cancellation while copying. Existing profile/updater signing/hash/channel, controls/permissions/notifications tests remain passing. Independent review separately passed all 28 focused lifecycle/updater/main-VM tests.
- Source Electron desktop checks passed. Fresh Windows x64 NSIS packaging, native package integrity and packaged desktop/permissions/admission/native-bridge checks passed. The new actual Electron quit fixture keeps an owned window/process alive during failed/pending cleanup, reports pending status and exits only after the holder completes. Its holder is fake; no native input is exercised.
- All five packaged capture fixture groups and all 14 packaged real-DTLS/fake-sink peer cases passed. The new main/updater/lifecycle ASAR bytes match source. Production npm audit reported zero vulnerabilities. Native sources and dependencies are unchanged from the baseline's passing 69-tests-per-configuration native/process/audit evidence; packaging rebuilt the locked default helper.
- Current unsigned local installer SHA-256: `7c533ff428277170e975c505d39d652903cb48037be051bb9dc81148c3617a7a`. This is build evidence, not installation or release acceptance. The current push must establish its own Windows CI result; it is recorded in Windows#1 after execution.

The future live coordinator must register actual authenticated child/release-holder completion; a direct guardian exit alone may leave surviving cleanup. No native coordinator/IPC, sensitive native UI guard, new mouse/keyboard/resource capability or production feature was activated by W1. Real OS input/stop, encrypted cross-client resource/media sessions, performance and installed-upgrade acceptance remain Not Verified. No private production signing key, installer execution, release signing/publication, tag change or deployment occurred.

The official companion docs/42 owner-disabled Actions exception was inspected: equivalent exact-candidate managed-host local package, real-service, Chromium/Firefox and ciphertext verification replaces its CI/dispatch requirement. Missing companion CI alone is not a blocker. Unexecuted, skipped and host-limited cases remain Not Verified. Windows Actions remain enabled and required for its exact pushed revision.

## Owned remote input transport, October 27, 2026

Starting Windows SHA: `9c683edc9b810a653165a28b7eca7b77ee7213d9`; companion remains `ae89e414ec71a193094ef7cbee5ad678418ff6f6`. The new transport is inactive and has no production caller, signing route or OS-input sink. It is a component checkpoint, not full Assist acceptance.

- `npm run check`: 143 tests and configured syntax checks passed. Six transport cases cover strict data-only SDP/candidates/session binding, expired proof without a watchdog callback, crossing the proof deadline while queued for only 20ms, and late native acceptance. The independent reviewer separately executed all six successfully.
- `npm run test:assist-peer`: all 14 actual Electron 44.7.2 source fixtures passed. They use app-owned helper/endpoint peers, real DTLS and a fake native sink. Cases include default/pre-canceled/malformed configuration, pre-acceptance discard, private-world/provenance isolation, retained cleanup, bounded early traffic, canceled admission/dispatch, approved-fingerprint mismatch, actual certificate substitution, wrong/extra channels, binary/oversized packets, a stalled native queue, crash and freeze. Some checks share one fixture case; the runner requires all fourteen case results.
- Review found and corrected proof freshness depending only on the watchdog, pre-admission traffic bounds and session-type coercion. Freshness is checked at IPC admission, queued dispatch and completion of asynchronous native acceptance. The 100ms queue limit starts when main receives the packet; it does not measure network/SCTP age.
- Fresh Windows x64 NSIS packaging, native integrity, packaged desktop/permissions/native bridge/admission and all five capture groups passed. All 14 peer fixtures also passed against packaged modules/preload under release file-protocol restrictions. Native source/dependencies are unchanged from the preceding 69-tests-per-configuration checkpoint and its passing exact-SHA CI.
- Current unsigned local installer SHA-256: `ccfe147a7a879c100248ceedcbb3b9fbd12ef28bf0c4d25a7420eb0edcd3a5e2`. No installer was executed. Both Windows build and protected release workflows now require the packaged peer fixtures before uploading/signing artifacts. The new push still needs its own exact-SHA CI result.

Live native coordinator/IPC, purpose-bound transport signing, host controls, native resource data plane, approved relay configuration and real Windows/cross-client acceptance remain unfinished. No physical input, real clipboard transfer, service session, production key, deployment, activation or release publication was exercised by this transport suite.

## Enrollment revocation and live capture routes, October 27, 2026

Pushed as `9c683edc9b810a653165a28b7eca7b77ee7213d9`. Exact-SHA Windows CI [40600104983](https://github.com/cortezdemartino/palisade-windows/actions/runs/40600104983) passed all required native/app tests, audits, packaging and packaged Electron/capture fixtures. This supersedes the failed historical run only for this new SHA; it does not qualify live native Assist or authorize a release.

Starting Windows SHA: `2f250f6836d1d23bc2228ea812d575d3369550ac`. Companion refreshed read-only at `ae89e414ec71a193094ef7cbee5ad678418ff6f6`; native bridge v1 remains unchanged. Risk: High. Independent source review found no new blocker in the disabled enrollment/stop and capture-route changes. It did not execute tests or authorize activation.

- `npm run check`: 137 tests and configured syntax checks passed locally, including current selected-route and native-binding negatives. Existing updater signature/hash/rollback, profile, permissions, controls and notification checks remain passing.
- `npm run test:assist-native`: formatting, strict all-target/all-feature Clippy and 69 Rust tests passed in each default/all-features configuration (46 library, 12 executor, four input and seven authority). Enrollment uses temporary DPAPI identities and synthetic decisions; stop tests use a fake input sink. No physical input or native dialog acceptance was performed.
- `npm run build:assist` and `npm run test:assist-process`: passed. Eight executable groups passed in each configuration; the explicit hidden-startup visibility case also passed. Process probes are non-authorizing even with sensitive features compiled.
- Production npm audit reported zero vulnerabilities. Cargo audit scanned the locked 34-crate graph against the fetched RustSec database with warnings denied and passed.

Historical CI for `2f250f6` failed in the final canonical capture fixture (`failed:route`, stream ended). That run is [40498962262](https://github.com/cortezdemartino/palisade-windows/actions/runs/40498962262), not passing evidence. The replacement reads the live selected ICE pair and requires reciprocal ports, both actual certificates and fresh native socket ownership. The exact Chromium address-redaction marker remains unknown metadata, never network-path proof. Exact-SHA CI must pass after the current push; its outcome is recorded in Windows#1.

Fresh Windows x64 NSIS packaging, native package integrity and packaged Electron controls/unread/permissions/native bridge/admission checks passed. All five packaged capture groups passed, including twelve negative/lifecycle cases and canonical native-bridge app-owned window selection, decoded pixels, one-time SID bind and source-stop. The bridge fixture also proves selectedRoute is absent from the page. Current route modules in the archive match source bytes. Local unsigned installer SHA-256: `bc86fbc421c72b25a2d94ff60a18b4b276a9da6909b5bb7ce9e9a0866a0a44b3`. This is local build evidence, not release or installation acceptance. Sixteen intentional Chromium bad-IPC renderer refusals in the admission suite retain their negative-test classification.

Native enrollment/runtime and capture changes remain inactive. Live coordinator/IPC, the owned remote input peer, canonical resource integration, real encrypted cross-client acceptance, physical emergency stop/OS input/clipboard/files and installed upgrade remain incomplete or Not Verified. No private signing material, release signing, installation, production activation or publication is included.

## Canonical capture checkpoint, October 27, 2026

Starting Windows SHA: f553f4e68a113a7743188c190b7e28b03ad43e07. npm run check passed 133 tests. Six new source/publication cases cover one-time tentative SID binding, exact operation/room/source retention, cancellation, stale completions and callbacks, fail-closed child invalidation, and delayed provider load/crash/failure/timer rejection without stopping a successor. Independent review separately passed 21 focused cases and cleared this inactive capture-only slice.

Windows x64 NSIS packaging, package integrity, packaged desktop/combined-preload checks and all five capture fixture groups passed. The new actual Electron/native-bridge fixture negotiates, selects an app-owned window through the existing picker, reads its operation-labelled video srcObject and decoded pixels, binds a synthetic SID once, refuses a second bind and observes source-close. No personal window was captured and no LiveKit publication was performed. The common preload is rebuilt from maintained sources before packaging/tests; production still uses the original preload with Assist disabled.

Unsigned installer SHA-256: 844a303d4dc8fcd606964d1ac77b0a94c0527fc84eb17a5264ee8c552bfc8159. This capture-only change retains the previous checkpoint's native implementation; its 58-test/native audit evidence is recorded below. The next native enrollment slice requires separate verification. No installer execution, private release signing or activation occurred. Exact-SHA CI is recorded after push; source/fixture success does not establish live encrypted cross-client viewing.

## Native bridge and trust checkpoint, October 27, 2026

Starting Windows SHA: a7d381b2b92d0bf7f23c9d4d81f4c72e6cdfd1da. Companion inspected read-only: ae89e414ec71a193094ef7cbee5ad678418ff6f6. Its native v1 validator remains exact blob 62f29f433dbc20de881108b635a97eee3bde6827 from 8f9c2a4; pinned fixtures preserve Git blob and SHA-256 provenance. No companion tests ran locally, and no Actions run was returned for ae89e41. Its mock-endpoint browser acceptance does not qualify Windows.

- npm run check passed: 127 tests, none failed/skipped. Fifteen native bridge/preload/contract cases cover exact schemas, enabled main-frame exposure, private live-document challenge/events, full-queue safety operations, timeout, reload, late signatures, replacement/terminal events, destroyed contents and canonical purpose-separated signature vectors.
- Native format/strict Clippy passed. All 58 Rust tests passed in each default/all-features configuration: 39 library, nine executor, four input and six authority cases. New tests verify exact four statement vectors, cross-purpose/tamper refusal, issuer qualification and actual temporary current-user DPAPI persistence without legacy/corrupt state replacement.
- Eight actual subprocess groups passed in each feature configuration, plus explicit hidden-startup visibility. Raw enrollment-challenge signing is no longer accepted. No probe injects input.
- Source and packaged Electron 44.7.2 desktop checks passed, including the new native bridge fixture with a fake coordinator. It proves the actual context-isolated surface, subframe denial, full-queue stop, stale-result rejection and reload/origin teardown without native enrollment or OS execution.
- Windows x64 NSIS packaging and package integrity passed with sensitive native features disabled. Packaged capture fixtures passed for isolated DOM video, real synthetic DTLS, ten negative/lifecycle cases and actual app-owned window pixels/source-stop. No personal screen/window was captured.
- Locked native and production dependency audits passed. Independent read-only review passed focused Node/signing/issuer tests and cleared the disabled component slice after stale lifetime/capacity findings were fixed.

Local unsigned installer SHA-256: 20d2107dd99e4794be7686c1d2ecd7accf7e7e4ba4f4a1e0a0e7cd3e106926f7. Exact-SHA CI is recorded after push; a7d381b passed [run 40448822734](https://github.com/cortezdemartino/palisade-windows/actions/runs/40448822734), which is previous-checkpoint evidence only.

The real issuer confirmation window, enrollment, native remote peer/coordinator, physical input, clipboard, cross-client encrypted transfer, installer execution and installed upgrade remain Not Verified. No production Assist pin is configured. No installer was run, private update key used, production feature activated or release published. This remains an inactive component checkpoint, not full Assist acceptance.

## Session-owned file checkpoint, October 26, 2026

Starting Windows SHA: 1234a184dcb76b7dbc81615e865ebfb8ed22f7bf. Local app checks: 112 passed. Native format/strict Clippy and 52 tests in each default/all-features configuration passed (33 library, nine executor, four input, six authority). The ten new session-owned file cases use signed authority and actual temporary NTFS files, with synthetic native file confirmation. They cover exact binding, direction/session/token misuse, cancellation/late selection, independent directions/input suspension, grant/lease/idle/session expiry, nonblocking close and shared-quota admission races. Independent review passed the initial 14 native file cases and approved the dormant component; the final suite also includes the suggested lease-only and simultaneous-quota cases.

Eight subprocess groups passed in each feature configuration and the explicit hidden-startup visibility fixture passed. Windows x64 NSIS packaging, native package integrity and packaged controls/capture fixtures passed. An initial assist smoke timeout prompted explicit hidden-startup show handling and phase diagnostics; final tests passed. The desktop runner now requires freshly written passing evidence from every subprocess, refusing stale output. No physical confirmation, real input or clipboard operation was exercised.

Local unsigned installer SHA-256: 2e2dba53145e91826a99eef15976cc01f2c67bfab6f2ec9c0eb9b73b6a5688ee. Current exact-SHA CI is recorded with the pushed checkpoint; the previous 1234a18 build passed run 40443288217. No installer was executed and no release was signed/published or native feature activated.

Companion 97b2a4f0d0b9766ab5e56197f405b36d5e83e959 addresses the two original service authority findings in independently reviewed source; companion tests were inspected, not executed here. Contract commit 8f9c2a45da7017ee36a9567f76413da53bd76075 publishes native bridge v1 and labelled statements. Windows is beginning that integration; these commits do not establish cross-client acceptance. See the active ASSIST-RELEASE-PLAN.md.

## Capture and permission-isolation checkpoint, October 26, 2026

Windows 10.0.26300 x64, Node.js 24.20.0, Rust 1.98.1, Electron 44.7.2. Starting commit: ffff0624b4b779814605530fd432e3143d19906d. Companion inspected read-only: c0778935a6a08303bf7a9d7de8f2fa75aa8ef536.

- npm run check: 112 tests passed, with no skipped or failed tests.
- Native format and strict all-target/all-feature Clippy passed. Rust tests passed in default and all-features configurations, 42 in each (23 library, nine executor, four input, six authority tests). Input sinks are simulated; owned-window/DPAPI/NTFS/lifetime APIs run natively.
- Eight actual subprocess groups passed against both the default release helper and all-features debug helper. The explicit hidden-process-startup visibility fixture passed. No probe path injects input.
- The real parent/holder ownership fixture kills the initial process and refuses repeated successors until the surviving holder exits. It proves kernel-object lifetime exclusion, not physical key release.
- The full service-view fixture renews a signed lease still listing locally suspended input. Input stays revoked while separately approved clipboard authority survives. Altered records are refused.
- The capture runner passed against source and packaged ASAR modules with release file-protocol restrictions: isolated DOM stream access, synthetic actual DTLS video, actual app-owned window selection/pixels/source closure, and ten negative/lifecycle cases. No personal application or entire monitor was captured. See ASSIST-CAPTURE.md.
- Packaged desktop checks passed, including the existing 30-case capture-denial fixture, controls, unread observation and description bridge. Expected renderer rejection diagnostics are negative-test evidence.
- Independent review separately passed 12 JavaScript capture/policy tests and 19 native authority/executor/ownership/polling tests. Reported stale-timer, liveness, ownership and polling findings were fixed. Approval covers dormant components only.
- Locked native and production dependency audits passed. Both workflow YAML files parsed.
- npm run package:win and verify:assist-package passed. The unsigned x64 NSIS artifact is out/Palisade-Setup-0.3.0-x64.exe. SHA-256: 055036b124f2a0cdcecf8da1e7c76e1d13d686f211d964fe0d33aecf2e142662.

An early owned-window fixture attempt exceeded its outer startup deadline; the fixture now allows the full selection budget and cold renderer startup. A packaged sequence also found a transient Windows image lock between test runners; the new runner retries only sharing errors for a bounded four seconds and still fails on other errors/deadline. Final packaged capture checks passed. Component liveness deadlines remain unchanged.

The previous ffff062 commit passed [Windows CI](https://github.com/cortezdemartino/palisade-windows/actions/runs/40418387443). The current candidate requires its own exact-SHA run; its result is recorded with the pushed commit in the issue evidence. Do not substitute the previous CI result.

No installer execution, authenticated-account upgrade, real remote input, live clipboard, physical stop shortcut, protected-desktop recovery, performance measurement or cross-client E2EE session was run. No release was signed/published or production feature activated. Full Assist remains Blocked, including unfinished Windows gateway/native IPC/resource/capture integration after the finalized companion handoff.

The following sections record earlier checkpoints and do not replace the current evidence above.

## Native stream/resource component checkpoint, October 26, 2026

Windows 10.0.26300 x64, Node.js 24.20.0, pinned Rust 1.98.1, Electron 44.7.2. Starting Windows SHA: c76d3370a770e811e4997d61cab738a9a9a90d3e. Companion source/fixtures: c0778935a6a08303bf7a9d7de8f2fa75aa8ef536, read-only.

- App regression/conformance suite: 104 tests passed, including both canonical protocol generations and real Ed25519 fixtures.
- Rust default/all-features checks compile the sensitive adapters and exercise only fake input sinks, owned hidden windows, temporary identities and temporary NTFS files. Idle grant expiry and Drop cleanup are tested. No real input or clipboard operation is performed by these tests.
- Seven actual subprocess groups passed on default and all-features binaries: guardian identity/bounds/liveness and executor readiness, no-input heartbeats, EOF, malformed frames, expiry, replay and idle parent loss. Probe paths remain non-authorizing in either build.
- Temporary filesystem tests passed for immutability, hash/size, no overwrite, MOTW, quotas, cancellation, parent rename refusal and OS cleanup on process exit without Rust Drop.
- Independent review ran 13 Rust and 12 JavaScript focused tests, all passed, and approved committing the disabled components. It did not approve activation.
- Locked Rust dependency audit passed with no reported advisory. Windows x64 NSIS packaging succeeded with default features. Final package integrity, Electron checks and exact-SHA CI are recorded after the candidate push.

No native input, physical emergency shortcut, live clipboard, cross-client session, installer execution, upgrade, protected-desktop recovery or performance benchmark was executed. Complete Assist remains Blocked, with integration and disposable-device qualification still outstanding. The installer is a build artifact, not a full Assist release candidate.

## Signing integration, October 17, 2026

- `npm run check`: 29 tests passed on Windows with Node.js 24.20.0, including predecessor-authenticated rotation, transition tampering, key-history rollback, untrusted keys, signed download locations, modified installers and both signing CLIs.
- `npm run package:win`: v0.2.2 Windows x64 NSIS build passed.
- The real external `C:\Users\azkar\palisade-update-private.pem` successfully signed the built installer metadata. `node scripts/verify-release.cjs out` verified all three signed manifests, both digests, size and version. The v3 download URL is constrained to the matching GitHub release path.
- The packaged ASAR contains public-key fingerprint `59337b8e1ba2145ff688c6674f5c09c633cc9c2b80e8ee57eab82eded3eaa781`, the public transition policy and no private-key or signing-script paths.
- Both workflow files parsed successfully. No hosted workflow run or public release was performed.
- Existing `v0.2.0` and `v0.2.1` tags pin the old key. The release listing was empty. The operator confirmed the only existing client is their test executable, approved replacing it manually, and does not have the old private key. This permits a manual bootstrap, not a fabricated signed transition. See UPDATE-SIGNING.md.
- Remote release-environment creation was rejected by automatic approval review because it changes shared controls and permits owner self-approval. No environment or secret was changed. Explicit approval is needed before configuring the reviewer, tag restriction and encrypted environment signing secret.

Local installer SHA-256: `119d4e8c87bce9dbd8836ff8d28a63c8f16659d6d2430a7db274d84cf1bc08a0`. The user still needs to perform the manual reinstall; live GitHub update delivery and authenticated-session preservation remain unverified. The existing installation and user data were not modified.

## Historical Phase 2 validation

Executed on Windows 10.0.26300 x64 on October 15, 2026, using Node.js 24.20.0 and npm 11.19.0. GitHub Actions uses Node.js 22; that hosted workflow has not yet been executed for this change.

## Executed checks

- `npm ci --no-audit --no-fund`: passed with network access outside the restricted sandbox.
- `npm run check`: 21 automated tests passed, including Windows helper execution; main-process and updater syntax checks passed.
- `npm audit --omit=dev --audit-level=high`: passed, zero production vulnerabilities.
- `npm run package:win`: Windows x64 NSIS packaging passed. Output is `out/Palisade-Setup-0.2.1-x64.exe`; Authenticode signing is optional and was not used.
- Both workflow YAML files parsed successfully. A hosted Actions run is still required.
- Release CLI tests generated temporary fixture keys, signed and verified both manifest formats, and rejected missing credentials, a mismatched key and modified artifacts. These tests did not use the production private key.
- Installer helper test compiled a harmless Windows fixture, executed the actual PowerShell helper, verified parent-exit ordering and NSIS arguments, and checked success/failure receipts and SHA-256 tampering rejection. No execution-policy bypass was used.
- Isolated NSIS v0.2.0 installation, v0.2.1 upgrade at the same path, synthetic profile-file preservation and test uninstall passed. This used a separate packaging identity, as described in UPGRADE-TEST.md.
- Repository inspection found an existing v0.1.0 installation on the workstation. The production identity was not installed over it.

The sandbox initially denied an atomic settings-file rename; the same test passed outside the sandbox. The executable-version test was corrected to accept Windows' four-part `0.2.0.0` representation. The helper test found a missing `Get-FileHash` cmdlet in the inherited module environment; the helper now hashes using built-in .NET cryptography and passes.

## Release status and remaining gates

No tag was pushed and no release was published. GitHub returned no published releases during inspection. A repository-scoped `PALISADE_UPDATE_SIGNING_KEY_B64` secret entry exists; its value was not read and its match to the pinned public key has not been established. The `release` environment endpoint returned 404. Configure required reviewers and an environment-scoped signing secret, then remove the broader repository-scoped secret after validating the protected setup. The workflow rejects publishing without reviewer protection, valid signing credentials and successful artifact verification.

Still unverified:

- Hosted Windows Actions build, production-key signing and publication of all four assets.
- Download and installation through the live public GitHub Releases endpoint, including v0.2.0 legacy-manifest compatibility in an installed client.
- Upgrade using the production application identity and a real authenticated Matrix device. Synthetic fixture preservation is not proof of authenticated-session preservation.
- Actual NSIS automatic relaunch and Windows-policy failures with the production application. The helper fixture test checks flags and receipts, but does not launch Palisade.
- Interactive tray/menu behavior, login startup, native website notifications, microphone permissions, external-link prompts and single-instance behavior on an installed client.
- Recovery from a partially replaced installation, tested in a disposable VM. There is no automatic binary rollback.

Follow UPGRADE-TEST.md before calling the production update lifecycle verified. Passing the automated tests and packaging is not a production release sign-off.

## Release preparation, October 17, 2026

The operator confirmed that all previous manual installations were uninstalled and authorized GitHub publication of 0.2.2. The external Ed25519 private key cryptographically matches the packaged trust anchor. All 29 tests and syntax checks passed outside sandbox restrictions; the production dependency audit reported zero vulnerabilities. The protected release environment has the repository owner as required reviewer and permits version tags only. The matching signing key was transferred directly to its environment secret without logging key material. Hosted build, publication and independent downloaded-artifact verification remain release gates. Old-key executables require a fresh manual installation; no authenticated old-key transition is claimed.

## Version 0.2.3 release candidate

The Windows client passes 34 automated tests, including manager dependency substitution, signed releases, downloads, IPC boundaries, settings actions, diagnostics and session/data preservation contracts. The source Windows UI smoke check passes with the release file-protocol restriction. The companion manager passes 40 tests and its packaged-source/public-key check. The production dependency audit reports zero vulnerabilities.

Windows packaging, packaged-archive UI verification, signed release artifact verification and hosted workflow results are checked again for the final commit. Interactive NSIS installation, an authenticated-session migration, native banner delivery and a real 0.2.0 to 0.2.1 upgrade have not been executed for this candidate. No passing result is claimed for those checks.

## Version 0.2.4 verification scope

The client passes 43 automated tests, including virtual-clock polling and retry schedules, authenticated update offers, cancellation, unread IPC sender validation, preview suppression, count aggregation and notification rate limiting. The Windows smoke suite now includes a real isolated preload fixture that changes unread labels, verifies baseline/read transitions and the red ring image, and verifies that no desktop bridge is exposed to the page. The source visual smoke check uses software rendering to avoid occlusion-dependent capture failures.

Real-account chat arrival, physical Windows taskbar rendering, native banner display, multi-hour sleep/resume and authenticated installation upgrades still require operator checks. Fixture success is not a passing result for those checks.

## Version 0.2.5

45 automated tests pass, including notification identity registration, development-runtime isolation, registration failures, foreground/background taskbar attention, focus clearing and notification throttling. The archive smoke suite checks the shell-readable notification logo outside app.asar. Visual Windows notification headers and physical taskbar flashing remain operator checks; no passing visual result is claimed.

## Version 0.2.6

45 automated tests pass, including system tray unread/reset behavior, a plain taskbar icon, and existing attention flashing. Physical tray rendering remains an operator check.

Windows x64 packaging and packaged-archive window/chat smoke checks passed. Local release manifests verified against the pinned public key and installer digests.

## Version 0.2.7

45 automated tests and Windows x64 packaging passed. Rebuilt PNG/ICO artwork was visually inspected with white shield markings. Packaged window/chat smoke checks passed on rerun after an unread-state timing assertion failed once. Local signed manifests and installer digests verified. Physical shell artwork remains an operator check.

## Version 0.2.8

45 automated tests and Windows x64 packaging passed. Visually inspected enlarged icons and bright red unread markings without a ring. Packaged window/chat checks passed after replacing short fixed unread waits with bounded state polling. Local release signatures and installer digests verified. Physical shell rendering remains an operator check.

## Version 0.2.9

45 automated tests passed, including separate web/control bounds, resize handling, fullscreen restoration and existing IPC security checks. Source and packaged Windows window/chat smoke checks and x64 packaging passed. The packaged check asserts that desktop buttons do not overlap the web view. Local signed manifests and installer hashes verified. Real-account visual layout remains an operator check.

## Version 0.2.10

45 automated tests passed, including immediate startup checks, no duplicate schedules, retries, overdue resume and cancellation of a startup request on shutdown. Windows x64 packaging, packaged UI/chat checks, and local release signature/hash verification passed.

## Version 0.2.11

45 automated tests passed. A new rendered root drag-region boundary assertion failed against the previous layout and passed after excluding the controls footprint. Source and packaged window/chat checks, Windows x64 packaging and local signature/hash verification passed. Physical mouse routing was not tested because the Computer Use runtime was unavailable; direct button actions and rendered geometry do not establish native input behavior.

## M4G-WINDOWS preparation — 2026-10-24

Status: Blocked by the missing committed M4G-WEB desktop contract. This is security preparation, not a working Assist host or qualified cross-client bridge. Audit reference: web 231e96b83230834c7419779c311236b3330dc3bc; Windows started at a72055f8270eca79a17d0b64b3b85c768fe65748.

- npm ci --no-audit --no-fund: passed with locked dependencies; existing transitive development-package deprecation warnings.
- npm run check: 49 tests passed on Windows x64, Node 24.20.0, OS release 10.0.26300. Existing signing CLI/key-rotation/hash, installer-helper, profile/settings, updater, tray, controls and notification tests retained.
- npm audit --omit=dev --audit-level=high: zero reported vulnerabilities.
- Source Electron 44.7.2 checks: controls, unread observer and synthetic permission fixture passed.
- npm run package:win: Windows x64 NSIS candidate 0.2.12 built successfully. Windows Authenticode remains optional and was not applied. No production release manifests were signed.
- Packaged ASAR checks: controls, unread observer and permission fixture passed. Synthetic microphone allowed and stopped; notification permission granted without a toast; camera, combined audio/video, display capture, same-origin subframe microphone, wrong-contents microphone and external navigation denied. Page has no Node/process/local-controls globals.
- Local candidate installer SHA-256: 61ba6166f02fa54bb3614ca089fb41b717c97e45bf5c9227e21262649a171217. This digest is local build evidence, not a release signature or installation approval.

The Electron fixture intercepts HTTPS with local test HTML and synthetic devices. It does not establish physical screen/window capture, installed-client upgrades, authenticated account/device preservation during an upgrade, real notifications, cross-client Assist authorization/E2EE, browser regression or performance. Those remain Not Verified. No OS screen, native input, real account, Matrix key or secret was accessed.

No tag, release, installation, production activation or web source change is included. Exact-SHA main CI must be read after the checkpoint push; its outcome is recorded in Windows issue #1 and the final handoff. See ASSIST-INTEGRATION.md and M4G-WINDOWS-REPORT.md.

## M4G-WINDOWS-FULL 0.3.0 candidate

- Starting Windows ef5dab42da44f5f31d2bfe2530f9f6ee2365d36c; canonical web contract 6c7ab894862ca3767b1239594e37c322e96ff2c4, protocol palisade.assist.bridge v1.
- npm run check: 65 tests passed locally, including the actual pinned web adapter, boundary validation, capture components and non-injecting lease/input/data helpers. Existing signing, update, installer/profile/UI tests retained.
- npm audit --omit=dev --audit-level=high: zero reported vulnerabilities.
- Source Electron 44.7.2 suite passed controls, unread, permissions and the new bridge/local picker tests. Production modern and legacy capture attempts were denied; local renderer termination was observed.
- An initial positive native fixture failed with NotAllowedError before the display handler. Instrumentation showed mediaTypes: []; pinned Electron source confirmed ambiguity with legacy desktop/tab capture. This remains a security blocker (#3), not a passing capture test. No empty-media workaround was applied.
- Final package/ASAR and exact-SHA CI evidence is recorded with the committed checkpoint in Windows #1. No release/activation is authorized by CI.

No actual native input, OS clipboard or file transfer, live MatrixRTC/LiveKit or data E2EE, real cross-client/browser regression, installed upgrade or workload performance was tested. No personal desktop source or private clipboard/file contents were captured. Production captureEnabled remains false. The earlier missing-v1-contract blocker is superseded by the committed web v1 handoff; expanded data protocol and native review remain open.

Final local 0.3.0 Windows x64 packaging and packaged-ASAR controls/unread/permissions/Assist fixtures passed. Local installer SHA-256: 90d3c144f8040c2be703b91bc83fc9fbffe4757075e25913bccada07eb7aea40. Authenticode and production release manifests were not applied. The passing suite includes intentional capture denial, not a passing native capture test.

## Capture-admission guard and resolution plan, 2026-10-24

Risk: High, permission boundary. Starting Windows checkpoint: ed6f1628a927c5e08ee8d8b71be8dd4b4b4091dc. Published web contract remains 6c7ab894862ca3767b1239594e37c322e96ff2c4; companion files were not modified. No runtime dependency, updater trust, persistent profile, production feature flag or release state changed.

- A new unit regression failed against the available-controller permission shortcut before the fix. Session policy now refuses display requests/checks and the display handler independently of controller availability; main also keeps the capability report unavailable independently of that flag.
- npm run check: 66 tests and configured syntax checks passed. Production dependency audit: zero reported vulnerabilities.
- npm run test:window-controls passed against source and the packaged ASAR: controls, unread observer, permission policy, canonical bridge/local picker and the new thirty-case admission fixture.
- The new fixture tests fifteen request shapes with controller availability false/true. Only the fake microphone request may actually receive a stream. An observer records the production policy decision and refuses every other grant before capture, so a failed negative test cannot capture personal content. It does not bypass permission handlers or disable TLS checks.
- Electron 44.7.2 and the official 45.0.0 comparison runtime both passed all thirty admission cases after the fix. Each run includes two stopped fake-microphone streams, twelve normal refusals and sixteen mixed legacy/microphone requests terminated by Chromium before the permission callback. Renderer termination is an explicit negative result, not a successful capture or graceful API rejection.
- On Electron 45.0.0 before the fix, a synthetic available controller caused four unexpected display permission grants: modern video, modern video/audio, legacy desktop video and legacy tab video. The observer blocked each before capture; the baseline production flag was never enabled. This demonstrates an unsafe future-enablement rule, not an observed production capture incident. The unit and runtime regressions pass with the guard.
- The comparison runtime was downloaded into ignored test output through the existing Electron downloader. It is not packaged or installed as the client runtime; Electron stays pinned to 44.7.2. Neither runtime established a safely selected positive capture path.
- npm run package:win passed for Windows x64 NSIS. Packaged main/session-policy/public-key bytes match source, all configured security fuses retain their required values, test/scripts are absent from the ASAR, and its only PEM is the public update key.
- Local candidate installer SHA-256: 57cfa8c62311cfeb79d112678b26c98db61e4f757a8f424b7739ee03616b4590. This is build evidence, not a release signature. No private signing key was read, production manifest signed, installer run, tag pushed or release published.

Fixture reports remain local under ignored out/. Exact-SHA Windows CI is recorded in Windows #1 after push. Real Matrix/Synapse/LiveKit integration, cross-client E2EE, selected-source capture, physical input/stop, clipboard/files, installed upgrades and performance remain Not Verified. This conservative denial change does not qualify the complete Assist product or waive the required approvals.

## Desktop 0.3.0 release authorization

The owner subsequently authorized publishing the desktop update while Assist capture/input/data routes remain disabled. Release preparation changes public documentation and adds docs/RELEASE-0.3.0.md; executable source, package version, dependency lock, profile paths, fuses and trust anchors remain those verified at 79e4d310c3cbb8f99077a3aa8aba6c6f05f9c4c9. The tag-based protected release workflow repeats locked tests, source/packaged boundary checks, audit, packaging and signature/installer verification on its exact revision before publishing. Final workflow and downloaded-asset verification evidence is recorded in Windows issue #1. Release authorization does not establish the unexecuted Assist or installed-upgrade tests above.

The owner then stopped publication because Assist remained disabled. No v0.3.0 release was published; its existing tag remains at e47ae036ba9714631d3cca5f805a0445c7726732. The subsequent objective is full Assist release testing, not retrying that partial release. A future candidate must use a fresh version/tag and satisfy the remaining feature/review/acceptance gates.

## Assist lifecycle and cancellation follow-up, 2026-10-25

Risk: High, offline authorization and stream lifecycle. Starting Windows revision: e47ae036ba9714631d3cca5f805a0445c7726732. Inspected published companion revision: 1e23a6ef243934fce10dc65255c0273c1afe814f; its protocol remains view-only. Companion source is read-only in this assignment. Native approval in project-palisade#33 remains pending.

- npm run check: 80 automated tests and configured syntax checks passed. Fourteen additional cases cover original-deadline refusal, expired/invalid-clock consent, reentrant local stop, granular/terminal cancellation ordering, idle expiry, malformed consent, stalled reads/writes, failed destinations, pre-source authorization and capabilities absent from the verified grant. Existing update/signing/profile/UI tests are retained.
- Focused tests demonstrated deadline/consent/stop/source-access failures before their fixes. Failed-destination and revocation-inside-authorization cases also failed before the stream-lifecycle repair. These components have no production operation route; no active native security incident is claimed.
- The lease now requires an independently authenticated original deadline, prevents verification-time stop from reactivating it, checks liveness before consent/admission, and provides irreversible per-capability and full-lease cancellation. Local in-memory consent is not a trusted native UI or device proof.
- File reception refuses before opening an unauthorized iterator, checks cancellation around authorization and writes, uses Node stream completion/error handling, and responds to scoped revoke while source/write operations are stalled. Tests use disposable in-memory streams; no filesystem path, clipboard, native input or network peer is accessed.
- Production dependency audit: zero reported vulnerabilities. No dependency, Electron version, app identity, session partition, update trust anchor or security fuse changed.
- Windows x64 NSIS packaging and packaged-ASAR controls/unread/permissions/Assist/admission fixtures passed. The packaged helper/main/policy/public-key bytes match source, configured fuses retain their values, tests/scripts are absent and the only PEM is the public update key.
- Local installer SHA-256: 1903c191faf46f871d7c9d6413c9357d34772afdbeea80ff34c7a52a01297ba5. This is build evidence for the current untagged source checkpoint, not an authenticated release or permission to reuse v0.3.0.

Exact-SHA CI and issue handoff evidence is recorded in Windows #1 after push. No native endpoint, real device verifier, encrypted data transport, positive selected-source capture, native input/stop, real clipboard/files, installed upgrade or full cross-client acceptance was executed or qualified. No production signing key was read, manifest signed, installer run, tag changed, release published or feature activated.

## Approved stream/group direction, 2026-10-25

The owner explicitly approved the native architecture and changed the intended product to stream-bound viewer/group requests and streamer invitations, host-selected permissions, up to two hours and terminal stream-end revocation. Approval was recorded in project-palisade#33; independent review, installation/activation and acceptance remain separate. Companion ee9c2763f19e62b405bbf9a9c97f187bc5a1291a now includes pairwise fifteen-minute control v1. The new model needs an explicit versioned amendment and shared fixtures.

This checkpoint changes requirements, review status and the Claude handoff only. Executable code and existing fifteen-minute validation are unchanged. Group requests, two-hour boundary cases, atomic controller selection and authoritative stream-end native revocation are Planned / Not Verified, not new passing tests. Prior 80-test and packaged-build evidence belongs to 6da6c5ef46f78d4a69f60caf7cc0a876dae095f8; exact-SHA CI for this documentation checkpoint is recorded in Windows #1 after push. No native functionality or release is activated.

## Canonical control conformance follow-up, 2026-10-25

- Pinned the unmodified canonical control v1 TypeScript source and shared JSON fixtures from companion ee9c2763f19e62b405bbf9a9c97f187bc5a1291a, with LF-normalized source SHA-256 provenance. They live under tests/fixtures and are not packaged. The shared issuer is a published fixture-only public key, not a production trust anchor; no private signing key was added.
- npm run check: 88 tests and configured syntax checks passed locally. Eight new conformance groups execute the actual canonical parser/Ed25519 verifier against all 33 shared grant/message fixtures, check unknown issuers and role directions, prove old v1 rejects two-hour/unversioned stream grants, and normalize a verified native fixture into the existing lease with separate local consent and exact endpoint binding.
- That normalization is test-only. No runtime bridge, native issuer/device verifier, endpoint key enrollment, OS input, transport, clipboard/filesystem broker or stream/group authority was implemented or activated. New-model positive two-hour and stream-end native acceptance remain Planned / Not Verified.
- Production code, dependency/package configuration, current fifteen-minute limits and signing/update trust remain unchanged from the prior verified source checkpoint. No redundant local installer rebuild was run for test-only additions; exact-SHA Windows CI performs packaging/boundary verification and is recorded in Windows #1 after push.

## Non-injecting native guardian, 2026-10-25

Starting Windows revision: 1880d08dec49e780f90878f86b1a3bcd4b21a54b. Canonical companion inspected: ee9c2763f19e62b405bbf9a9c97f187bc5a1291a. Risk: High, native process/identity and packaging boundaries. Independent review approved the non-injecting component after hotkey, lifecycle, parent-identity, parser and notice findings were resolved.

- npm run check: 89 tests and existing syntax checks passed, including updater signature/rotation/hash, profile preservation, controls and notification regressions.
- npm run test:assist-native: formatting, Clippy with warnings denied and 10 Rust tests passed. Tests execute the real canonical signatures/negative fixtures, duplicate/missing/unknown fields, wrong issuer/endpoint/generation/deadline and old v1 two-hour refusal. Real current-user DPAPI persistence/signature verification and corrupt-file refusal run on a temporary test identity, not the installed profile.
- npm run test:assist-process: five real executable groups passed. They exercise handshake/status/stop, valid issuance remaining unauthorized, tamper/untrusted-key/binding refusal, wrong parent/normal-mode parent image refusal, malformed/oversized/incomplete/duplicate/replayed frames, probe signing refusal, pipe EOF, actual parent exit, missing/partial handshake, idle parent and undrained-output backpressure. The latter confirms fewer than all queued responses were completed and termination at the watchdog deadline. No input, clipboard or capture operation is available.
- cargo audit 0.22.2 scanned the locked native graph against the fetched RustSec database with warnings denied; passed with no advisory findings. Production npm audit reported zero vulnerabilities.
- Native lifecycle tests register Windows session notifications and post synthetic messages only to their owned hidden windows. They do not lock/suspend the PC, register/test the production global hotkey or establish actual emergency stop/secure-desktop/input behavior.
- The binary is compiled from locked Rust sources and packaged with dependency/Rust runtime license texts. Native manifest integrity and archive/source equality checks passed; only the existing public update PEM is packaged and required Electron fuses remain unchanged. Final installer and packaged Electron evidence is recorded after completion below and in Windows #1.

Production main/preload do not launch the guardian or expose native operations. The description report remains endpoint absent. Every issuance diagnostic returns authorized false. No production Assist issuer is pinned, no service grant acknowledged, no Matrix token extracted and no release key read. Full capture/control/clipboard/files, source/stream authority, full two-hour group workflows, physical stop, actual lock/secure-desktop transitions, installed upgrade and cross-client acceptance remain Blocked / Not Verified. No installer execution, production manifest signing, tag/release publication or activation was performed.

Final local packaging evidence for this guardian checkpoint: Windows x64 NSIS build, npm run verify:assist-package and packaged Electron controls/unread/permission/Assist/admission suite passed. All 30 admission negatives passed; sixteen intentional Chromium bad-IPC renderer refusals retain their previously documented classification. Helper bytes match the locked source build, archive main/policy/public-key bytes match source, dependency/runtime licenses are present and required fuse values remain intact. Local installer SHA-256: 7ef05bc1713341aebfee7647d09edc178557ac079cc26a1cbc10ff1ded03a739. This unsigned local test build is not a GitHub release or installation acceptance. Exact-SHA CI follows the main push.

CI follow-up: acfae87 failed at process startup on the hosted administrative runner, before audit/packaging. The production guardian correctly refused its token. The reviewed fix permits the existing token only in diagnostics-only --probe mode, with no identity/resource/input operation and all other creator/SID/session/pipe checks retained. A native predicate test proves normal mode refuses every elevated combination. New CI must pass before this revision is reported green. The companion advanced to 2d2dcc1970d087f3a44f2f7d40e4d8d5121a0db0; its four proposed native operations are acknowledged in the handoff, but no exact liveness/stream-v2 authority was delivered.

The probe-only follow-up passed local formatting/Clippy, eleven Rust tests and all five real executable groups. It adds a normal-mode elevation-denial test and fixed-code startup diagnostics. Native release/installation/UIPI acceptance remains unexecuted; hosted diagnostic success will not qualify those boundaries.
