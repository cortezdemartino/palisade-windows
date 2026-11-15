# Endpoint-wide authenticated lease ordering

Starting revision: `aee6f48a00ec2869611955bf96b8adb4698edaa7`.
Companion inspected read-only: `24b722c206f86651378aeae447252ea71977823a`.

## Defect and implementation

The existing service `streamAssist.ts::endpointLease` stores `lastLeaseAt` on the
endpoint. Windows previously kept the increasing request timestamp only inside
each session's LeasePoller. Two sessions could sign the same millisecond, or send
higher times before an earlier delivery, causing a legitimate request to fail
with P_REPLAY and end that session. Resource bootstrap followed by a poller needs
the same endpoint-wide ordering.

The launch's existing EndpointIdentity now owns one LeaseLane. It serializes
lease request allocation and HTTPS delivery for all its sessions, without holding
a session gate or any input/Stop lock. No second grant registry or signer exists.
The native clock supplies time; same-millisecond requests increment by one, with
a hard one-second drift limit. Wall-clock rollback, arithmetic/format exhaustion
and poisoned ownership fail closed. Failed or uncertain sends retain their stamp.
Cancellation is checked before waiting, after acquiring the lane, and after the
response. Queued canceled work cannot start a network operation.

The authenticated statement, body and fixed HTTPS endpoint are unchanged. No
renderer timestamps, credentials or configurable URLs are accepted. TLS,
revocation checks, request bounds and redirect refusal remain unchanged.

LeaseResponse retains the actual request-start Instant after serialized waiting.
The gate still verifies the signed response, original deadline and monotonic
20-second limit, and rejects expired or ended authority before applying it.
Waiting for a request lane cannot extend the preceding lease. Urgent Stop remains
independent through native atomics and exact executor events; retained cleanup
can wait for the bounded HTTPS owner without mistaking cancellation for cleanup.

## Verification

Four deterministic lane tests cover concurrent sessions/delivery order,
failed/canceled stamp retention, cancellation following a false pre-lock snapshot,
and rollback/exhaustion/bounded drift. No sleeps or real network are used.
All four passed; the seven identity tests (including real current-user temporary
DPAPI files and operational key isolation) passed. Strict all-target/all-feature
Clippy passed. Independent source review found no blocking replay, deadline or
Stop issue and independently passed the four lane tests. Its suggested post-lock
cancellation coverage improvement was incorporated and passed.

`npm run test:assist-native` passed formatting/strict Clippy and 255 tests in each
default/all-feature configuration. The app code and dependency locks are unchanged
from the separately verified recovery candidate. Exact pushed-SHA Windows
process/packaging CI is recorded in Windows #1. This is native source/unit verification, not a real-service lease, resource
bootstrap, enrolled endpoint or physical-input acceptance result.

## Next connected resource work

The existing companion service supports resource-only endpoint-lease bootstrap;
resource grants have endpoint:null, are already active, and receive no native
grant.offer or service ACK. Bootstrap must validate the complete signed view and
native capture track, then retain exactly one existing session runtime/gate and
adopt the first lease/facts before starting its regular poller. First refresh must
preserve strictly increasing service lease issuance and the existing five-second
schedule.

Resource transport needs independent certificate/source/lease readiness, up to
one ordered control/clipboard/file channel each, and an authenticated connection
lifetime distinct from terminal input generations. Input-only Stop must retain
independently live resource transport; it cannot rely on resource_peer_verified
alone. A resource-only peer must never emit remote-control-active.

Reuse native Prompt and SessionFiles; do not silently consent fetched grants.
Protected dialogs require positive input/cleanup quiescence. Extend existing
stores for multi-file queues, final verified Save, private interrupted staging,
ten-minute resume policy and fresh per-resume consent. Add bounded typed bulk
pieces without widening all IPC or starving safety; 64KiB base64 chunks exceed
the current private frame bound. Replies stay on the originating grant/lane.
Retain exact transfer/sync bridge lifetimes for late progress/status. Enforce the
canonical one-second text sync rate and genuinely decoded PNG limits rather than
advertising the existing header-only helper as image support.

These are unfinished Windows features. The two companion frontend corrections
are documented in the [handoff](../ASSIST-COMPANION-HANDOFF-PROMPT.md). All resource
flags and production Assist remain off. Cooperative physical delivery stays
Blocked; all real-device/cross-client acceptance remains Not Verified. No release,
signing, installation or activation is authorized by this change.
