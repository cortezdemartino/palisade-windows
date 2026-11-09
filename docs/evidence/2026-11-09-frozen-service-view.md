# Frozen service lease-view compatibility

Starting Windows revision: `74f93d234d0964763240e8a399ed75846d74036c`.
Companion revision inspected read-only: `01ae58565dd29dce692bace6d09834c5efdbff69`.

## Defect and source verification

The native poller's `View` used `deny_unknown_fields` but omitted three fields
emitted by the frozen service: `streamTrackName`, `approved` and `inputRequest`.
Consequently an otherwise valid current endpoint-lease response would fail parsing.
The earlier poll regression built a legacy-shaped response and missed this defect.

Independently recalculated Git blob and SHA-256 from the exact committed bytes:

| Companion source | Git blob | SHA-256 |
| --- | --- | --- |
| `services/admin/src/streamAssist.ts` | `d9a5cc45e6f49042d3a43dfa84cc0e62731b442e` | `e60015377d58dab5e3388f2220364b022685b349db0885c2688969598d9d7886` |
| `services/admin/src/app.ts` | `f33961ce15f22e093829be45846167a89e367284` | `42ee7b9938016c132dc5c76386cfa7db7862bbbbc0d31267b5252b2e11226956` |

The service's `sessionView` emits these fields; its endpoint-lease route wraps
that view as `{session: ...}`. This is a Windows parser correction, with no wire
change, canonical fixture modification or companion code change.

## Correction and trust boundary

The existing parser now recognizes the typed descriptive fields. Legacy absence
remains supported; nullable capture name/request remain nullable. Unknown fields,
including unknown nested request members, remain refused. Approved capabilities
must be unique known capabilities; a pending request must contain unique input
capabilities from that list, its bounded canonical ID and an in-session creation
time. Those facts never call consent or peer verification or create a grant.

Every signed record and lease still goes through the existing exact binding,
signature, immutable deadline and revocation checks. Facts are published only after
the complete response passes. Cancellation checks surround publication/retrieval;
failure and every worker exit, including unwind, clear stored facts. The snapshot
is descriptive status, not an authorization/readiness API.

The full signed-poll regression now includes the actual frozen fields and still
proves that locally revoked input cannot return through service metadata, while
independently authorized resources remain usable. Additional tests cover legacy
absence, a pending request without any authority, 15 malformed metadata variants
and snapshot cleanup during unwind. Independent source review also checked the
exact service shape and cleared this inactive parser/status boundary; it ran no
builds, service requests or device tests.

## Verification and limits

Formatting/strict Clippy, **143 Rust tests per default/all-feature configuration**
(93 library and 50 integration), **202 application tests**, **26 no-input process
groups per configuration** and hidden-startup visibility passed. Local NSIS packaging
and packaged native integrity/cleanup probe passed. The preceding direct-witness
candidate passed the full packaged desktop/capture/peer checks locally; the new
exact-SHA CI runs them again for this candidate.

Unsigned development installer SHA-256:
`673b749ac3a9fc2f90be8f9280856dbc293abee42e6dc19829ea18bd2f13c7fa`.
Packaged helper SHA-256:
`3e74e0e58551cd50c1fd8bd666d62b57307d8fe7aa8dd60672178aeae3bde958`.
These artifacts were not installed, signed or published.

Classification: **Implemented / Verified in simulation** for parser compatibility
and signed-authority regression. No real-service or cross-client test ran here.
`streamTrackName` alone does not prove capture: the future owner must corroborate
its exact live native operation, source and requested track name against the current
signed session/lease before input or resources. That binding, persistent runtime,
fresh acquisition, native extension resources, cooperative physical arbitration,
positive crash-surviving cleanup and device acceptance remain unfinished.

No production feature switch or published desktop release changed.
