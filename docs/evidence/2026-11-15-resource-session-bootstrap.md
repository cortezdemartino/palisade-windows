# Native resource-only session bootstrap

Starting revision: `7c2bbf50d91c2429ad77eff5faefc8afff14233c`.
Its [exact-SHA Windows CI](https://github.com/cortezdemartino/palisade-windows/actions/runs/43690388205)
passed. The earlier recovery candidate `aee6f48` also passed
[exact-SHA CI](https://github.com/cortezdemartino/palisade-windows/actions/runs/43689437148).
No installer was published or installed.

## Connected implementation

A missing native session can now be initialized from the existing authenticated
endpoint-lease route. Trusted main invokes a closed private-v3 ResourceSession
request on the ordinary eight-slot lane. The endpoint checks the owned document,
confirmed enrollment, issuer, qualification mode and actual native capture before
requesting the snapshot using its operational signer and shared request lane.
No URL, account token, approval flag, path or clipboard content comes from the page.

InitialLease is explicitly untrusted metadata until complete verification. Native
binding comes from the enrollment and capture room/stream plus the selected signed
record. The new gate verifies all resource records, the whole service response and
signed covering lease, and the expected native capture track before runtime
threads start. Selected resource directions must be service-active, endpoint:null,
and one of the four clipboard/file capabilities. Pointer/annotation/input grants
cannot substitute as resource transport authority.

Exactly one Endpoint.sessions registry, StreamRuntime, SessionGate and LeasePoller
retain that session. A retry uses the existing source-bound runtime; ended sessions
remain tombstones. The first verified lease/facts are adopted before polling;
refresh retains the existing five-second cadence and enforces cancellation and
expiry while waiting. Serialized queue delay does not extend any old lease or the
original deadline. Failure before initialization creates no live permission.

This is session/connection preparation, not OS-resource consent. Resource grants
are already service-active and get no synthetic grant.offer or service ACK.
resource_transport_grant cannot authorize clipboard/filesystem operations: those
still require the existing authorize/local-consent/per-operation checks. A build
without native-input explicitly refuses input admission, consent and peer setup,
even if the snapshot contains descriptive signed input records.

The main adapter retries native prepare once after this bootstrap. It does not
create a fake input READY or active lifecycle event. Resource-only DTLS readiness,
clipboard/file channels and the eight resource actions remain unfinished. All
related capability flags stay false, so this checkpoint is not a working transfer.

## Cancellation and ownership

The private request gets an immutable ingress scope, bounded ordinary capacity and
urgent CancelNative cancellation. Native checks cancellation/document/source after
network work; the source's independent Stop witness remains part of the runtime.
Input-only Stop does not cancel resource bootstrap by advancing its input revision.
Global Stop, renderer loss, source end and the existing native cleanup owners
retain their terminal behavior. The private reply contains no service snapshot or
resource payload. Frozen native/control/extension page protocols are unchanged.

## Verification

- App tests: 269 passed, zero skipped; canonical extension reproduction and syntax
  passed. Frozen fixtures were not changed.
- Formatting and strict all-target/all-feature Clippy passed. Native tests passed
  258 cases each in default, all-feature and resources-only configurations.
  The resources-only configuration is now part of test:assist-native and CI.
- Six focused lease tests passed, including valid signed resource bootstrap with
  no native input/local consent and refusal of tampered signatures, wrong service
  owner/source, missing/issued grants and over-age response budgets.
- The native transport-purpose test covers all four resource directions and
  rejects pointer/annotation authority without granting OS consent.
- Thirty focused Node orchestration/channel tests passed. Bootstrap is not an ACK
  or a fabricated control connection; native refusal remains a refusal.
- Independent source review and the same focused tests found no remaining
  blocking finding. It confirmed the four-capability whitelist and resources-only
  input guard corrections. This is source/unit review, not full Assist review.
- Rebuilt no-authority Windows process suites passed 42 cases in default and
  42 with all features compiled; the hidden-startup fixture passed. The Probe
  bootstrap test refuses before HTTPS/resource authority and keeps the channel
  usable. Windows NSIS build and both helper/update package-integrity checks
  passed. Packaged controls/permission checks passed30; isolated capture passed
  12 negatives/lifecycle cases plus synthetic/owned-window DTLS; input transport
  passed14 fake-sink cases. Unpublished installer SHA-256:
  `f9719b173caee8336be7d3d56c9c9a4ef65549091e8e2116b3998ede0249e3df`.
  Exact pushed-SHA CI is recorded against the commit in Windows #1/#4.

No real endpoint/service bootstrap, clipboard/file operation, physical input or
cross-client acceptance ran. All 30 real acceptance cases remain Not Verified.

## Remaining integration

Implement independent resource transport/connection readiness and shared channel
liveness through input-only Stop. Then connect protected per-direction/action UI,
text/PNG clipboard, multi-file queues, safe private interrupted staging, explicit
verified Save, fresh resume consent, one-second text synchronization, app-owned
drop surface, bounded bulk framing and retained transfer/sync event lifetimes.
Reuse SessionFiles and the current brokers rather than replacing them.

The companion still needs the exact stale-loss/conditional-revoke and input-only
Stop corrections in [the handoff](../ASSIST-COMPANION-HANDOFF-PROMPT.md). These do
not replace the unfinished Windows work. Production issuer, TURN, Firefox,
disposable devices, integrated review and release approval remain separate gates.
Cooperative physical delivery stays Blocked. Public Assist remains inactive.
