# Resource-only and shared native peer integration

Campaign M4G-ASSIST-FINAL-6, Subphase 2 continuation. Starting Windows revision
`7dceeb2` (inventory commit following baseline `ca983d0`). Companion inspected
read-only at `74e8fa8525677f7f0cba8d3106f00a7e65f3c042`.

**Integrated connection plumbing; resource actions remain unfinished.** This is
not complete A1, working clipboard/files, physical input acceptance, or a release
candidate. All public resource/cooperative flags stay false. No account secrets,
production issuer, OS clipboard/file content or personal desktop input was used.

## Implementation

The existing PeerPermit now has an explicit input/resource purpose. Resource
setup requires the exact signed endpoint:null resource direction and a covering
lease in the independently verified current service view. It requires no fabricated
input grant, input READY, service ACK or resource OS consent. Sealing is still the
existing purpose-limited endpoint transport signature. Actual observed DTLS
certificate admission for resources sets only resource connection verification;
it does not set input verification, input age state or local resource consent.

One retained StreamRuntime/SessionGate/LeasePoller still owns the session. There
is no additional authority registry or executor. Shared input peers may carry an
independent resource connection permit, with a channel allowlist derived from
currently signed clipboard/file directions. Default builds cannot mint resource
transport authority; resources-only builds cannot mint input authority.

Each connection has an atomic, exact-attempt cancellation witness. The safety
owner retains it and the original optional InputRevoker. An old Close cannot
stop a successor or revoke independent resource grants. Gate resource action
authorization checks the admitted witness rather than a lingering verified
Boolean. Replacing the admitted witness cancels the displaced connection first.
Future resource jobs must additionally carry their exact original peer witness;
they must never adopt a replacement's current session Boolean.

Before observation the selected resource setup grant remains exact. After
admission, another current signed resource direction in the same immutable
binding can sustain the connection if the original direction is revoked. Actual
actions remain separately permissioned; a file grant cannot authorize clipboard.
Full session/source/document/lease loss keeps terminal behavior.

Trusted main and the sandboxed gateway now distinguish connection READY, input
authority and actual executor READY. A shared connection can become resource-ready
while its input worker starts. Early input gestures are bounded and discarded,
never queued for later replay. Input-active lifecycle is published only after
actual input readiness. Resource-only connection or closure emits no input
lifecycle event. This avoids falsely activating or revoking control in the web UI.

Input-only Stop synchronously fences main/preload input admission and the native
generation while retaining a verified independent resource association. The
bridge's trusted retainResources marker preserves that peer lifetime through
input-scoped Stop and suspended status. The hosted page cannot set this marker.
Global Stop, source loss, navigation and application shutdown still own and cancel
the entire connection. A fresh input generation requires a fresh peer; no claim
of uninterrupted file transfer across that replacement is made.

The existing isolated DTLS gateway accepts at most one ordered reliable control,
clipboard or file channel, limited by the native permission-derived allowlist.
Certificate observation no longer requires a control channel. Text and canonical
64-KiB binary chunk envelopes have bounded transport queues independent of the
reserved input release lane. No resource payload crosses the hosted bridge.

Resource payload dispatch deliberately remains a refusal until native brokers,
protected action UI, transfer framing and status ownership are connected. A fake
sink proving encrypted transport is not evidence of clipboard/file functionality.
The current main adapter closes refused resource traffic rather than executing it.

## Verification and review

- App/syntax/extension reproduction: 272 tests passed, zero skipped.
- Native default, all-feature and resources-only: 262 tests passed each, zero
  ignored; formatting and strict all-target/all-feature Clippy passed.
- Four new native tests cover resource-only certificate admission without OS
  consent/input, exact setup versus post-admission direction independence,
  input Stop, cancellation/replacement, wrong certificate/gate and expired lease.
- Thirty-nine focused Node tests cover the bridge/runtime/private vocabulary,
  resource-only reconnection after input Stop, shared worker startup, early-input
  discard, scoped lifetime preservation, and exact result fields.
- Eighteen real Electron DTLS fixtures passed with synthetic data/fake native
  sinks, including resource-only certificate verification, wrong channel refusal,
  shared resources after an input fence, maximum binary chunk envelope and
  duplicate-channel refusal. No native OS action was injected.
- Independent focused source review and separate Node/native resource filters
  passed. Review found and corrected premature input fencing during worker
  startup, early-input teardown, and resource-only input-lifecycle emission.
  This is component review, not the integrated #33 security gate.

Initial failed local iterations remain in `.git/assist-resume/final6-*`: strict
test-shape/Stop regressions and a fixture returning an unclonable function were
corrected. Passing source/unit/transport evidence does not supersede failed or
unexecuted device acceptance. All 30 real cases remain Not Verified.

## Next executable work

Connect exact-attempt resource message validation, native protected per-direction
and per-action consent, clipboard text/PNG decoding/Copy/Save, SessionFiles private
staging/chunks/whole-file SHA-256, multi-file/drop/resume/sync, outbound encrypted
messages, progress/status lifetimes, urgent cancellation and authenticated ICE.
Do not advertise a transport-only connection as an available resource feature.

The companion still needs stale-peer conditional revoke, input-only Stop and
safe channel-capability reconciliation. Native resource grants/consent alone do
not prove resource channel continuity. See the [handoff](../ASSIST-COMPANION-HANDOFF-PROMPT.md)
and [master matrix](../ASSIST-COMPLETION-MATRIX.md).
