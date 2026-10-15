# Validation

## Completed locally

- Node.js 22 policy and release tests: 11 passed.
- Main process, updater and module syntax checks: passed.
- Release manifest signature validation: valid, altered and untrusted-key cases tested.
- Release source and redirection rules: tested with controlled network responses.
- Installer hash, length and partial-download cleanup: tested with local fixtures.

## Required on Windows

- Locked dependency install and dependency audit.
- Electron Windows installer packaging in GitHub Actions.
- Install over 0.1.0 without losing the Palisade Matrix device profile.
- Application startup, login, notifications, voice microphone permission and session recovery.
- GitHub Release signed-manifest distribution from the public repository.
- Install prompt, unattended installer completion, relaunch and new-version checks.
- Interrupted downloads, missing release assets, installer failure and rollback handling.

Passing these tests does not establish an independent security assessment or production approval.
