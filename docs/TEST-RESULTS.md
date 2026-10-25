# Validation

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
