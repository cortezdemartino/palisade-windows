# Companion Assist qualification handoff

Continue in `cortezdemartino/project-palisade` on main. The Windows repository is
read-only in that assignment. Start from current main, retaining candidate
`5fd1958358e2a56d965a4952c35eb6281d461b3e` and evidence
`ea9f927e90389677fd7a41a86fffc808027d1ce3` as inspected checkpoints.
Windows implementation checkpoint:
`2f8f258edc7b3f03b863e6af9984eb4def8f9ef2`. Companion readiness/evidence updates
through `3a75ffed2fb38a49578717a590a4fae1c7ae3735` retain the same frozen wire.

The cooperative contract is already frozen. Do not recreate it, change the nine
fixture families, weaken the 30 acceptance cases or advertise unsupported native
features. Windows still owns native source binding, normal executor cleanup
receipts, SessionGate/LeasePoller attachment, consent qualification and real input.
This is not a request to bypass those Windows implementation requirements.

## Required correction: stale input loss must not revoke a successor

Inspected companion `24b722c206f86651378aeae447252ea71977823a` read-only.
The three affected sources are unchanged from `060a8afa`. Keep Windows read-only
in the companion assignment. Do not change frozen native/control/extension wire
formats, fixtures or acceptance cases to solve this race.

Reproducer: an active mouse grant G is replaced by an issued grant G+1. A poll can
skip the intermediate revoked view. `streamAssistController.ts::syncNative`
computes `wanted = false` while G+1 awaits native acknowledgement. That branch
closes the old peer without first retiring its callback incarnation. Its closed
callback calls `inputLost`, which reads the *current mutable* session grants and
revokes control_mouse, including issued G+1. A delayed capability-only revoke HTTP
request can also arrive after G+1 has become active. Windows cannot repair this
service mutation from its native endpoint.

Required work in `project-palisade`:

1. Retire/detach the old callback incarnation before every intentional close,
   including `!wanted`, session replacement and teardown. Capture the original
   session, endpoint, exact grant IDs and generations for peer-loss callbacks;
   never derive their revoke targets from a later mutable session view.
2. Add a local intentional-retirement path that does not send the existing
   unscoped `native-bye`. `NativeInputPeer.close()` currently always sends it.
   A queued old bye/lifecycle cannot identify a successor's generation under
   frozen v1. Treat ambiguous notifications conservatively and refresh current
   native/service state instead of revoking unrelated fresh authority. Native
   enforcement remains authoritative; do not infer that silence grants input.
3. Add an additive, conditional service revoke request with expected grant ID and
   generation checked atomically at mutation time. A mismatch must leave the
   successor intact. Frontend identity checks alone leave an HTTP race. Retain
   authorization checks and existing deliberate current-session Stop behavior.
4. Regression-test G-active -> G+1-issued without a revoked intermediate poll;
   old closed/lifecycle/bye before and after G+1 activation; a delayed old revoke
   HTTP request; genuine current-peer failure; same-generation late mouse or
   keyboard acknowledgement; and independent clipboard/file grants.

Inspect `apps/web/src/features/assist/streamAssistController.ts` (`syncNative`,
`inputLost`, `onNativeLifecycle`, native-bye handling),
`apps/web/src/assist/control/nativePeer.ts::close`, and
`services/admin/src/streamAssist.ts` plus the HTTP revoke route. Run the existing
risk-based tests and real-service browser regressions, update Windows #4 with
exact commit/test evidence, and keep production Assist gates unchanged. No
production deployment or activation is requested.

The old-close callback issue was reproduced by the independent reviewer using
the actual controller with an in-memory peer fixture. It was not a browser or
real Windows acceptance run. All 30 native acceptance cases remain unqualified.

## Required correction: input Stop must not stop all native work

At the same inspected companion revision, `streamAssistController.ts::stopControl`
promises that resources and screen sharing continue, but calls native `stop`
without a session. Frozen docs/49 defines that operation as ending all native
Assist work. Windows correctly treats it as global launch/source/resource Stop.

Change this input-only publisher action to the existing session-scoped
`peer.close {session}` safety operation, followed by conditional input revocation.
Keep deliberate global/emergency Stop separate. Confirm both actions retain their
actual documented scope. No frozen message fields need to change.

The helper's `nativePeer.ts` also closes its entire peer and every resource channel
when the control channel closes. Coordinating input-only Stop must therefore
retain the authenticated shared association while input is fenced, or correct
that handler so independently authorized resources stay alive. Windows still has
matching resource transport/lifecycle engineering to implement; a retained grant
or Boolean alone is not proof of a live authenticated resource channel.

Regression: active input plus an accepted file/clipboard operation, publisher
Stop Control, immediate refusal of further input, ongoing authorized resource and
screen activity, then a true End Assist/global Stop that ends all of them. Do not
count a mock-only result as real Windows acceptance.

## Historical requested qualification harness addition

The current native WIN-P01 test calls `publisher.share()` and waits for decoded
capture before inspecting enrollment. Windows' first connected endpoint supports
native enrollment while capture, input and resources remain unavailable. Add a
separately named, explicitly staged native enrollment check using the existing
canonical native-v1 methods and authenticated service enrollment/proof routes.
Do not count this stage as passing WIN-P01, WIN-C01 or full Assist acceptance.

Inspect `apps/web/src/features/assist/streamAssistController.ts::enrollEndpoint`,
`apps/web/e2e/assistAcceptance.spec.ts` and docs/49 and docs/54. Reuse their existing
account/device/service authorization. If a current publisher stream is required,
state that prerequisite accurately and use only the approved disposable fixture;
do not fabricate a production stream or loosen device binding to remove it.

The stage should verify the real Windows bridge/helper, the artifact's exact
revision/hash/profile, native operational public key, independent issuer pin,
local confirmation, canonical signature and the service's proven endpoint. Accept
truthful false feature declarations. Test denied/canceled enrollment, device or
endpoint substitution, restart/replacement and stale proof rejection. Never inject
OS input, use MOCK/native approval booleans, extend authority or disable TLS.

Document the required independent test issuer fingerprint, approved disposable
Windows device and test account coordination in the existing qualification guide
and interventions. Keep credentials/private keys out of issues and profiles.
Record actual outcomes separately from source/unit/mock evidence. Return the exact
commit and test commands to Windows #4. No wire changes, deployment, installation,
production activation or release publication are authorized by this handoff.
