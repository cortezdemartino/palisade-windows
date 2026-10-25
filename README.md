# Palisade for Windows

Palisade's Windows desktop client runs the existing Palisade service in a restricted Electron window. Messaging, workspaces and encryption use the deployed Palisade application and the Matrix SDK. The Windows client handles desktop integration, application permissions and installer updates.

**Development version: 0.3.0, unpublished.** Windows 10/11, x64. Assist screen sharing and remote control remain disabled. The client is under active development and has not undergone an independent security audit.

## Features

- Dedicated application window, single-instance behavior and system tray
- Existing Palisade account and Matrix device support
- Sandboxed web renderer without Node.js integration; narrow versioned Assist description API
- HTTPS navigation restrictions and confirmation for external links
- Microphone audio and notification permissions; other device permissions denied
- Automatic update checks through [GitHub Releases](https://github.com/cortezdemartino/palisade-windows/releases)
- Ed25519-signed release metadata and SHA-256 and SHA-512 installer verification

The canonical Assist v1 description bridge and inactive local picker/controller are implemented. Capture remains disabled pending the Electron permission review. Remote input, clipboard/files and cross-client E2EE remain blocked or unverified; isolated planners/validators do not constitute a functioning remote-control endpoint. See the integration guide and review packet.

## Install

Download the most recent installer from [Releases](https://github.com/cortezdemartino/palisade-windows/releases). Run it on Windows and sign in to your existing Palisade account. The existing test client must be replaced manually with v0.2.2 to establish the new signing-key trust anchor; see the signing and recovery guide.

Windows may display an unknown-publisher warning for unsigned installers. This project does not disable certificate validation or bypass Windows security policy.

## Build

Install Node.js 22.13 or later, npm, the Visual Studio C++ linker and the Rust toolchain pinned in rust-toolchain.toml. Packaging builds the inactive native guardian from locked sources. From the repository root:

```powershell
npm ci --no-audit --no-fund
npm run check
npm run test:assist-native
npm run build:assist
npm run test:assist-process
npm run package:win
```

The installer is written to `out/Palisade-Setup-0.3.0-x64.exe`. Local builds do not receive an official update manifest. Official release manifests require the separately held release signing key.

## Updates

The installed app checks for updates immediately when ready and every six hours. **Check for updates** is also available in the application menu and tray menu. The client verifies available release metadata using its pinned public key. Confirmed installation opens the bundled update manager, which downloads and verifies the installer before installation. Polling itself never downloads or installs.

The signing key is separate from Windows Authenticode. A GitHub account is not required to download updates from this public repository. If signature or integrity checks fail, the current version remains installed.

The repository contains no backend service, web assets or Matrix authentication implementation. Changes to Palisade's website are outside this project's scope.

## Documentation

- [Windows Assist contract audit and preparation](docs/ASSIST-INTEGRATION.md)
- [Assist resolution plan and engineering defaults](docs/ASSIST-RESOLUTION-PLAN.md)
- [M4G-WINDOWS checkpoint report](docs/M4G-WINDOWS-REPORT.md)
- [User guide](docs/USER-GUIDE.md)
- [Build and release operations](docs/OPERATIONS.md)
- [Signing, authenticated key rotation and recovery](docs/UPDATE-SIGNING.md)
- [Security notes](docs/SECURITY.md)
- [Release notes](RELEASE-NOTES.md)
- [Phase 2 audit](docs/AUDIT-PHASE-2.md)
- [Upgrade test procedure](docs/UPGRADE-TEST.md)
- [Validation and release gates](docs/TEST-RESULTS.md)

## License

The Windows client source is available under the [Apache License, Version 2.0](LICENSE). Copyright 2026 Onyx Operations Group. The Palisade name, logo and other brand identifiers remain the property of their respective owner; no trademark license is granted. The separately hosted Palisade service and website are not covered by this repository's license. See [third-party notices](THIRD-PARTY-NOTICES.md).

The inactive [native guardian](docs/ASSIST-NATIVE-GUARDIAN.md) now provides protected endpoint identity, canonical signature diagnostics, process/watchdog isolation and binary integrity verification. It is not started or advertised by the application. Full Assist still requires the canonical native bridge, independently authenticated lifecycle, safe capture and Windows/cross-client acceptance.
