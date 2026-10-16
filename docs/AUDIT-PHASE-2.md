# Phase 2 audit

Reviewed the main process, security policy, update modules, key and signing scripts, dependency manifests, both Actions workflows, tests, documentation, licensing and packaged assets at baseline `bea6a0b`.

| Finding | Resolution |
| --- | --- |
| Updates supported only SHA-512 | Added a v2 signed SHA-256 manifest; retained the exact v1 format for installed v0.2.0 clients. |
| Installation was considered started as soon as the executable spawned | Added an external helper with startup handshake, parent-exit wait, final hash verification, NSIS flags and installation-result receipt. |
| Interrupted downloads could leave temporary executables indefinitely | Confined downloads to a disposable profile subdirectory, cleaned on next startup; partial transfers are removed on failure. |
| Exclusive-create failure could remove a pre-existing file | Downloader removes only a file it created. |
| No visible progress, release notes or background failure status | Added taskbar/menu progress, bounded plain-text notes, native update notifications and persistent menu status for the running instance. |
| Deleted postponed installers caused repeated failures | Missing cached installers now trigger a fresh download. |
| Tray setting was volatile and applied only to close | Persisted the preference and applied it to minimize and close. |
| No login option or About dialog | Added native menu entries; login registration is opt-in. |
| Multi-command PowerShell steps could mask earlier failures | Added explicit native exit-code checks. |
| Release job had write permissions during dependency installation | Separated read-only build/sign from artifact verification and publishing with write permissions. |
| Release environment had no required protection | Workflow now rejects publishing unless the environment has required reviewers; repository configuration remains outstanding. |
| Lockfile root had an outdated license label | Aligned it with the existing Apache-2.0 license. |

Preserved package name, product name, application ID, NSIS identity derivation, installation options, pinned public key and `persist:palisade-desktop`. No new runtime dependencies, renderer bridges, backend changes or certificate-policy overrides were added. Existing website notifications continue through Electron's native notification support.

The implementation does not provide automatic binary rollback. Windows policy can block an unsigned installer or helper. Installation-result receipts and manual recovery are documented. Production signing and authenticated-session upgrade validation remain release gates, as detailed in TEST-RESULTS.md.
