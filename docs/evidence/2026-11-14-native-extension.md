# Native extension and Profile 2 routing

Starting revision: `fc2c6f013a2214421110686721c49a47f1751e57`.
Its [exact-SHA Windows CI](https://github.com/cortezdemartino/palisade-windows/actions/runs/43659693485)
passed, including native, process, installer, packaged isolation, capture and
isolated DTLS checks. This clears the earlier peer checkpoint's CI prerequisite,
not real Assist acceptance.

## Implemented

The existing two-member page surface now carries both frozen native v1 and the
separately negotiated extension. `ext.hello` retains its version 1 envelope and
selects only an offered v1/v2/v3. Native fixes that choice before the first session;
neither reconnection nor a later offer changes an existing runtime's input profile.
Request/replay/expiry identifiers and event sequences are separate by protocol.
Ordinary and safety budgets remain shared and bounded. Resource cancel/sync-off
cannot cancel a peer or input grant merely because it has the same session ID.

Trusted-main extension validation is a reproducible CommonJS port of the frozen
`extension.ts` blob `005ac6ce6ca4b7c4fc27bcc32aa6d9d5f7ad03e1`.
`node scripts/sync-assist-extension.cjs --check` is part of `npm run check`.
V1/v2/v3 fixture tests cover the production port as well as the reference source.
Private dispatch uses a closed Rust enum for the eight canonical resource actions;
it accepts no paths, clipboard bytes or approval flags from the hosted page.

The same immutable native declaration governs both `input2` advertisement and
Profile 2 selection: non-Probe, existing compiled `native-input` feature, and an
independently pinned disposable qualification namespace. An extension version by
itself cannot activate the decoder. Public/default/Probe builds declare false
and retain Legacy. A separate unpublished qualification build can route the
existing Profile 2 executor through the authenticated peer path. It still needs
approved disposable-device acceptance; this does not authorize its activation.
Normal packaging continues rejecting a helper compiled with native input enabled.

## Verification

- `npm run check`: 262 tests passed, zero skipped; port/syntax checks passed.
- Native default and all-feature configurations: 237 tests passed each.
- Strict all-target/all-feature Clippy and formatting passed.
- Rebuilt Windows helper live-process suite: 27 passed, zero skipped, including
  real extension negotiation and prompt safety refusal with eight blocked ordinary
  requests. This uses Probe mode and grants no native authority.
- Independent review: port reproduction and 34 focused Node tests passed. The
  initial version-only Profile 2 selection was corrected to the exact immutable
  feature declaration. No new blocking source finding remained in this slice.
- Real process/packaging/CI results for this increment are recorded against the
  pushed revision in Windows #1/#4. They are not physical-input evidence.

## Remaining work and classification

Negotiation, strict routing and qualified-build Profile 2 integration are
Implemented / unit verified. No full real native Assist session was executed.
The 30 real acceptance cases remain Not Verified.

Resource brokers/channels, retained resource progress lifetimes and cleanup event
propagation are unfinished. All resource/cleanup/localYield feature flags remain
false. Resource operations return `unsupported` or `not-active`; safety requests
use the reserved native reader path. They never return fake transfer IDs, access
the real clipboard or fabricate cleanup completion. Fresh-grant reacquisition
also remains Windows engineering work. Cooperative physical delivery stays
Blocked independently. No release, installation, signing or activation occurred.
