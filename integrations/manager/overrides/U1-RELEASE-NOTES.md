# U1 release-note source integration

This is an unreleased source candidate. The published Manager 1.0.1 executable and
the desktop's pinned Manager asset have not been replaced. Do not overwrite that
published asset with a build from this source.

The maintained updater modules now match the Windows client: `release.cjs`,
`key-trust.cjs`, `notes.cjs`, `notes-history.cjs` and `downloader.cjs`. Both online
and offline discovery verify v3 release manifests, including authorized signing-key
transitions, exact download location, SHA-256 and SHA-512. Legacy schema 1 remains
available only when modern metadata is absent and the installed transition history
is empty. Invalid modern metadata never falls back to older metadata.

Release details display the shared structured `release-notes-v1.json` document.
The document has its own Ed25519 signature, a distinct signing domain and a digest
of the exact authenticated v3 installer payload. Unsigned GitHub descriptions are
never displayed as trusted release notes. Unavailable or invalid notes are labeled
unavailable and do not bypass installer verification.

Verified notes are cached, reverified on load and shown in a version selector.
Only plain text is rendered. Cache entries cannot supply a root key, signing-key
history or installation authority. Historical display may verify a pre-rotation
release against the portion of the packaged history active at that release;
current update discovery always requires the entire packaged history.

For an offline bundle include the unchanged installer, `release-v3.json`,
`release-notes-v1.json`, and the existing legacy manifests when older clients need
them. Never include signing keys. Missing editorial notes remain supported.

Run `npm run check`, `npm run package:dir` and
`node scripts/verify-package.cjs` with Node 22 before creating a new Manager
candidate. A package build is not installed-upgrade acceptance or permission to
publish. The desktop-owned installer helper is a separate U1 work item.

The Windows repository carries a content-checked source integration kit under
`integrations/manager`; use its application script to reproduce these changes.
