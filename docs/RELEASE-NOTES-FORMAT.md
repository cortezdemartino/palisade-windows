# Authenticated release notes

Release notes use a separate signed `release-notes-v1.json` asset. The existing
strict release v3 payload and all older signature formats remain unchanged.

Author a reviewed draft in `release-notes/<package-version>.json`, starting with
`release-notes/template.json`. Run `node scripts/release-notes.cjs` to validate it.
Packaging and signing refuse a missing draft, a version mismatch or unchanged
template text. Validation checks structure and limits, not whether editorial
claims are true. Maintainers must review every claim against implemented behavior
and actual evidence. Never derive release notes automatically from commit subjects.

Draft schema 1 has exactly `schema`, `version`, `locale`, `title`, `summary`,
`sections` and `compatibility`. An empty title uses the application's default
heading. An empty compatibility list omits that section. Sections have exactly
`category` and `items`; supported categories are `new`, `improved`, `fixed`,
`security` and `limitations`. Include only nonempty categories, once each. Locale
uses a bounded language or language-region code; no automatic translation is
claimed. Text is normalized Unicode plain text, without control characters,
direction overrides, HTML or external links.

Limits: 120 characters for an editorial title, 600 for a summary or change item,
five categories, 12 items per category, six compatibility entries of 400
characters, and 64 KiB per signed document. Both interfaces use text-only rendering.

The signer adds product `org.onyxops.palisade`, platform `win32-x64`, stable channel,
the exact v3 publication timestamp, and `releaseSha256`. That digest is SHA-256 of
the canonical v3 **signed bytes**, including its existing domain prefix. It binds
the notes to the installer version, filename, location, size and both hashes.

The notes envelope has exactly `payload` and `signature`. The Ed25519 signature
covers `PALISADE-DESKTOP-RELEASE-NOTES-V1`, a newline, and the UTF-8 canonical JSON
payload. Property order is defined by `normaliseNotes` in
`src/updates/notes.cjs`. The signer is the active key authorized by the verified
release's transition chain. Notes cannot introduce a key or reset installed trust
history. Original root and legacy installer manifests remain compatible.

`scripts/sign-release.cjs` signs the sidecar alongside the three existing installer
manifests. `scripts/verify-release.cjs` refuses absent, mismatched or invalid notes
before release publication. The release workflow uploads the sidecar explicitly.
This tooling does not authorize production signing or publication in phase U1.

The local Settings section **Updates & What's New** displays the installed/latest
version, signed summary, expandable details and a version selector. Manager's
source candidate uses the same validators and editorial format. Both retain at
most 12 signed entries in a 1 MiB display-only cache, reverify them on load, and
reject altered entries. Cached pre-rotation notes use the packaged transition
prefix active at their historical version. Current update authorization always
uses the full installed prefix. History never authorizes installation.

Missing or invalid notes are shown as unavailable. The installer signature and
hash checks remain mandatory. GitHub's unsigned description is never a substitute
for authenticated notes. An optional notes timeout cannot suppress an otherwise
verified update. Offline notes are available only after authentic signed notes
have actually been cached or supplied with a verified offline Manager bundle.
No historical editorial summaries are fabricated.

If notes are unavailable, retry when online or obtain the complete signed offline
bundle. Do not edit a cached document, change a public key, disable verification,
or re-sign with an unapproved key to repair an error. Keep signing material outside
the source and assets. Follow `docs/UPDATE-SIGNING.md` for authorized rotation and
lost-key recovery.

The published Manager pin is unchanged. Its existing binary does not gain these
features from a source-only change. See `integrations/manager/README.md` for the
reviewable source integration and its separate candidate verification procedure.
