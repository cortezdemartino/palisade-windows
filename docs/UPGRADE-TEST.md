# Reproducing the v0.2.0 to v0.2.1 upgrade

## Isolated NSIS installation test

This test changes the packaging identity to `org.onyxops.palisade.phase2test` and product name to `PalisadePhase2Test`. It verifies NSIS replacement at the same path and preservation of a synthetic profile file. It does not launch the application or validate a real Matrix session. It refuses an existing test installation or profile. The real Palisade installation is not a target.

From the repository root in PowerShell, after `npm ci --no-audit --no-fund`:

```powershell
New-Item -ItemType Directory -Path out -Force | Out-Null
git archive bea6a0b --output=out/baseline.zip
Expand-Archive -LiteralPath out/baseline.zip -DestinationPath out/baseline -Force
& .\node_modules\.bin\electron-builder.cmd --projectDir out/baseline --win nsis --x64 --publish never --config.appId=org.onyxops.palisade.phase2test --config.productName=PalisadePhase2Test --config.directories.output=../upgrade-test/baseline
& .\node_modules\.bin\electron-builder.cmd --win nsis --x64 --publish never --config.appId=org.onyxops.palisade.phase2test --config.productName=PalisadePhase2Test --config.directories.output=out/upgrade-test/candidate
pwsh -NoProfile -File scripts/test-windows-upgrade.ps1 -Execute
```

Check each build's exit status before continuing. The script installs v0.2.0, verifies its executable version, installs v0.2.1 at the same path, checks the profile fixture, then uninstalls the test application and removes its synthetic profile. If interrupted, inspect the `PalisadePhase2Test` installation before rerunning. Do not remove an existing Palisade profile.

## Production identity and GitHub delivery test

Use a disposable Windows x64 VM and a dedicated Palisade test account. Do not replace a primary workstation installation for this test.

1. Build or obtain the original v0.2.0 installer. Install it at the default path, then repeat the test using a custom installation path in a fresh VM snapshot.
2. Sign in with the test account. Record the Matrix device identity, verify the device, receive a message and close/reopen Palisade to establish persistence. Record the installation path and profile path without exporting tokens or cryptographic material.
3. Configure the protected `release` environment and its matching signing secret. Run the reviewed v0.2.1 tag workflow and verify all four release assets. Do not substitute a fixture key for the production key.
4. From v0.2.0 choose Check for updates. Its v1 verifier must accept `release.json`, check SHA-512 and offer v0.2.1. Choose Later once, then check again and install. The v0.2.0 handoff does not explicitly request relaunch, so open Palisade manually if NSIS does not relaunch it.
5. Confirm About shows v0.2.1, the install path is unchanged, and the same authenticated Matrix device and encrypted-message history remain usable without signing in again.
6. Enable tray behavior and launch on login. Verify minimize, close, Quit, second-instance activation and Windows login. Disable login startup afterward. Verify native message/update notifications and audio-only microphone access, while camera and screen capture remain denied.
7. Publish a separately reviewed, higher test release to exercise v0.2.1's v2 verifier, taskbar/menu progress, release notes, postponement, helper handshake, automatic relaunch and next-start receipt handling. A stable client must never be directed to an experimental release.
8. Interrupt a download, remove its staged file, disconnect the network and retry. Validate that signature, hash, size and downgrade failures never produce a runnable update. Use a local test fixture for deliberate tampering; do not publish invalid production assets.
9. Block helper execution using the VM's normal policy and confirm the current application stays open. Exercise an installer failure and an interrupted installation in a VM snapshot. Confirm the next launch reports failure and the official installer can recover without deleting the profile.

Only mark production update delivery and authenticated-session preservation as passed after executing these steps. The isolated test and unit tests are separate evidence.
