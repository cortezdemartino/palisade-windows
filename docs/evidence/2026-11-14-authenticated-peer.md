# Authenticated native peer integration

Starting Windows revision: `974b61a7575fc93e1de36257cac79ff5cf392333`.
Companion inspected read-only: `060a8afa320163de14c02f75b048b89abaddbf27`.
Canonical candidate: `5fd1958358e2a56d965a4952c35eb6281d461b3e`.
The six source blobs, nine fixture families and 30-case manifest are unchanged.
Manifest SHA-256: `f9ac9d835333e341907ae63f2d2a497ef4944607ea60fc344e81ad64115606b0`.

## Implemented path

The existing canonical `peer.offer` now connects the app-owned isolated DTLS
gateway to the retained native runtime. Native preparation requires an admitted
input grant, current consent and a covering signed lease. An opaque witness binds
the exact gate instance, input epoch, grant and generation. Resource authority
cannot prepare this input transport. Preparation never sets peer verification.

Only the gateway's own certificate can be sealed through the private main/native
method. Native signs the frozen endpoint transport statement, with the retained
session/device/endpoint/helper binding. Actual remote certificate observation waits
for that exact attempt's seal. Native verification enables executor preparation;
the gateway admits packets only after the native worker reports ready. Native
authority and source checks continue on every packet and health check.

The peer handle references the existing gate, lease facts, Stop flags and input
worker. It does not create another grant registry or executor. Its bounded IPC
lane avoids blocking consent dialogs. Private v3 reserves eight ordinary, four
peer, four safety/pulse and one terminal response slot. Ordered gateway dispatch
reserves four release entries without overtaking earlier input.

Source cancellation closes only peers bound to that source operation, including
late preparation replies. Document loss, peer loss and native refusal close
transport admission and signal the existing input revoker. Stop acknowledgement
means fenced authority, not completed child/mirror cleanup. Launch ownership stays
with the existing monitor and lifecycle holder. Peer-worker join failure cannot
skip endpoint cleanup or become a terminal success on retry.

Trusted-main packet arrival uses the pinned Electron `hrtime.bigint` value.
Native backdates its monotonic instant using QPC, conservative libuv rounding and
tick allowance. The actual Electron 44.7.2 / Node 24.21.0 / libuv 1.52.1 runtime is
checked against native QPC without authority or input. Sender-age baselines use
arrival rather than worker dequeue time. Valid stale packets still reach native
sequence validation; only execution is discarded. Owned releases remain subject
to current authority and exact native ownership.

## Verification and review

- Node application suite: 256 passed, zero skipped.
- Native all-feature suite: 234 passed, zero ignored.
- Strict all-target/all-feature Clippy and formatting passed.
- Rebuilt Windows helper live-process suite: 26 passed, zero skipped. The new
  blocked-ordinary-lane case proves prompt refusal without native authority.
- Actual isolated Electron DTLS component: 14 passed, fake native sink, no OS input.
- Independent source review: 26 targeted Node tests passed. Identified dialog
  contention, source-stop races, pre-native stale drops, clock-rounding refusal
  and cleanup ordering were corrected. No remaining concrete blocker was found
  in the Legacy peer slice. This does not approve integrated Assist or release.

An initial process run used the previously compiled helper while its replacement
was still building: 25 passed and the new operation failed. After rebuilding,
all 26 passed. No security check was relaxed. Final package/CI evidence is tracked
with the pushed revision in Windows #1/#4.

The first pushed run, [43658729634](https://github.com/cortezdemartino/palisade-windows/actions/runs/43658729634),
failed because the new real-Electron clock test could not open its executable
on the runner. Both workflows now explicitly prepare/check the locked runtime
before native tests. The fixture canonicalizes its repository path and sets its
working directory. Local clock tests passed after the correction; the check was
not skipped or weakened. The next exact-SHA run must pass before CI is cleared.

## Classification and remaining work

This is implemented application/native peer routing with unit, real-process and
isolated DTLS component evidence. It is not an actual enrolled Windows/helper
session. All 30 real native acceptance cases remain Not Verified.

Profile 2 negotiation, fresh-grant reacquisition, resource transport/dispatch,
native resource consent and real-service qualification remain engineering work.
Cooperative dispatch remains Blocked on physical delivery arbitration. Public
input/resources/localYield remain false; no feature switch, issuer, installation,
release version, release artifact or production service was changed.
