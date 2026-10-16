# Palisade for Windows

Palisade's Windows desktop client runs the existing Palisade service in a restricted Electron window. Messaging, workspaces and encryption use the deployed Palisade application and the Matrix SDK. The Windows client handles desktop integration, application permissions and installer updates.

**Source version: 0.2.1 (unreleased).** Windows 10/11, x64. The client is under active development and has not undergone an independent security audit.

## Features

- Dedicated application window, single-instance behavior and system tray
- Existing Palisade account and Matrix device support
- Sandboxed web renderer without Node.js integration or an IPC bridge
- HTTPS navigation restrictions and confirmation for external links
- Microphone audio and notification permissions; other device permissions denied
- Automatic update checks through [GitHub Releases](https://github.com/cortezdemartino/palisade-windows/releases)
- Ed25519-signed release metadata and SHA-256 and SHA-512 installer verification

## Install

Download the most recent installer from [Releases](https://github.com/cortezdemartino/palisade-windows/releases). Run it on Windows and sign in to your existing Palisade account. The 0.1.0 client must be updated manually once; automatic update checks start with 0.2.0.

Windows may display an unknown-publisher warning for unsigned installers. This project does not disable certificate validation or bypass Windows security policy.

## Build

Install Node.js 22 or later and npm. From the repository root:

```powershell
npm ci --no-audit --no-fund
npm test
npm run package:win
```

The installer is written to `out/Palisade-Setup-0.2.1-x64.exe`. Local builds do not receive an official update manifest. Official release manifests require the separately held release signing key.

## Updates

The app checks for updates shortly after startup and every six hours. **Check for updates** is also available in the application menu and tray menu. When an update is available, the client verifies the release manifest using a public key embedded in the application, downloads the installer from the GitHub release, checks its hash and size, then asks for permission to install it.

The signing key is separate from Windows Authenticode. A GitHub account is not required to download updates from this public repository. If signature or integrity checks fail, the current version remains installed.

The repository contains no backend service, web assets or Matrix authentication implementation. Changes to Palisade's website are outside this project's scope.

## Documentation

- [User guide](docs/USER-GUIDE.md)
- [Build and release operations](docs/OPERATIONS.md)
- [Security notes](docs/SECURITY.md)
- [Release notes](RELEASE-NOTES.md)
- [Phase 2 audit](docs/AUDIT-PHASE-2.md)
- [Upgrade test procedure](docs/UPGRADE-TEST.md)
- [Validation and release gates](docs/TEST-RESULTS.md)

## License

The Windows client source is available under the [Apache License, Version 2.0](LICENSE). Copyright 2026 Onyx Operations Group. The Palisade name, logo and other brand identifiers remain the property of their respective owner; no trademark license is granted. The separately hosted Palisade service and website are not covered by this repository's license. See [third-party notices](THIRD-PARTY-NOTICES.md).
