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
