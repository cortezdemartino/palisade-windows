# RC-2 persistent endpoint checkpoint

Starting Windows revision: `3bebe84065fc92de50f3c74658976d11a3919234`.
Implementation revision: `2f8f258edc7b3f03b863e6af9984eb4def8f9ef2`.
Companion inspected read-only at `ea9f927e90389677fd7a41a86fffc808027d1ce3`,
including candidate `5fd1958358e2a56d965a4952c35eb6281d461b3e`.
The frozen nine fixture families and all 30 acceptance cases are unchanged.
Manifest SHA-256 remains
`f9ac9d835333e341907ae63f2d2a497ef4944607ea60fc344e81ad64115606b0`.
The control, cooperative and extension source blobs independently match
`29bfd8d51d8cf423932379add25b83ae06334d8c`,
`caaadb956475a91c2b7e1776651d7c7a8a88f873` and
`005ac6ce6ca4b7c4fc27bcc32aa6d9d5f7ad03e1` respectively.
The yield fixture SHA-256 remains
`5e678a011977aa063bba30200234ae9bb0fd971196daf198143e208155b74e89`.
Companion main advanced during implementation to
`3a75ffed2fb38a49578717a590a4fae1c7ae3735` with readiness/evidence updates.
Its frozen source blobs and manifest were rechecked and remain identical.

## Connected implementation

The application now contains a real coordinator-backed canonical native bridge,
enabled only by an explicit disposable qualification build. Public desktop builds
still expose no native endpoint. There is one launch reservation per application
lifecycle. The native monitor also refuses concurrent endpoint monitors for the
same exact application PID/creation time. A preempted kernel-object name can deny
service; it cannot confer authority.

The existing hierarchy, exact-image verification, restricted inherited handles,
non-killing Windows job, direct Stop event, independent watchdog and bounded
queues are reused. Private version 3 adds typed canonical native operations.
Versions 1 and 2 retain their original vocabularies and cleanup scopes.

The persistent endpoint owns one lazily created memory-only operational signer,
qualified issuer and locally confirmed enrollment tuple. It retains attempted
endpoint IDs, refuses replacement/replayed enrollment, checks the native account,
device, endpoint and canonical challenge, and exposes only purpose-limited proof
signing. Matrix credentials and authenticated service HTTP remain in the existing
web enrollment flow. Local `status.endpoint.enrolled` records native confirmation;
the web's independent `endpoint.proven` check establishes service acceptance.
Neither boolean grants a capability.

Navigation, renderer death, canceled native work, global Stop and Windows security
events end this launch's native channel. Restarting the application is currently
required for a replacement endpoint. No old document can adopt the signer.
Stop remains independent of the ordinary dispatch queue. The endpoint revokes its
record and discards its signer before its dedicated `endpoint-only` terminal.
The monitor requires that exact scope/challenge, exact coordinator exit and job
zero before emitting `endpoint-and-process-exit`. The Electron lifecycle holder
also waits for the exact monitor process closure. None of these scopes prove
native held-input release.

The real Electron test found that the initial blank document navigation ended an
otherwise unbound runtime. The corrected bridge invalidates initial witnesses
without terminating that unbound launch; every established document remains
terminal on navigation. The test now exercises renderer crash cleanup through
the actual compiled helper, rather than a mock dispatcher.

## Truthful refusals and unfinished RC-2 requirements

All native feature flags remain false, in default and all-feature binaries.
`localYield` is not advertised. Capture operations refuse with
`capture-unavailable`; peer offers/candidates refuse with `transport-mismatch`;
grant offers refuse with `not-enrolled` or `capture-unavailable`. Safety close,
cancel and Stop remain available. No grant acknowledgement, lease, secure peer,
capture, resource or input success is fabricated.

The existing StreamRuntime/SessionGate/LeasePoller are not yet attached to this
endpoint. A real native source binding and qualified normal executor custody
remain prerequisites. The normal ProcessInput terminal receipt channel and
monitor-owned mirror settlement are still unfinished. Probe receipts, process
exit and job zero cannot substitute for them. RC-2's primary positive signed
grant/consent/ACK/lease test therefore remains Blocked, not passed.

Existing app-owned enrollment dialogs reject page approval flags and late results,
but their Win32 button handler is not proof against a same-integrity local process
synthesizing a button command. The independent reviewer identified this limitation.
No claim of physical or synthetic-proof consent qualification is made. The
default-disabled, all-sensitive-capabilities-false boundary is retained.

## Disposable qualification build

`node scripts/build-assist-qualification.cjs <public-profile.json>` produces an
unsigned unpacked application with a separate application ID, userData/session
partition and native DPAPI namespace. It does not run or install that application.
The profile contains exactly `origin`, `issuerFingerprint` and `namespace`.
Use an independently obtained test issuer fingerprint; never paste private keys
or Matrix credentials into the profile. HTTPS and normal Windows TLS/revocation
checks remain mandatory. Production origin is explicitly forbidden in this profile.

The build embeds the same public configuration into ASAR and the native binary.
It never accepts runtime environment/page trust overrides. Production packaging
rejects the qualification profile and independently checks the packaged native
build policy. The build script removes the temporary source profile on completion
or failure. Default updater, account storage and app registration remain unchanged;
the isolated qualification application has no updater, registration or login setup.

On an explicitly approved disposable VM:

1. Provision the isolated service, normal TLS trust and test Matrix accounts using
   companion docs/54. Obtain its issuer fingerprint over an independent channel.
2. Create the public profile with an HTTPS origin, its 64-character lowercase
   SHA-256 fingerprint and a separate lowercase namespace, such as `device-test`.
3. Build on Windows x64 with Node 22 and the pinned Rust toolchain. Record source
   revision, public profile, ASAR/helper hashes and the helper's `--build-info`.
4. After separate device approval, launch the unpacked qualification application
   on that VM, sign into the disposable account and use the canonical enrollment
   flow. Compare the native account/device/issuer/key display and approve locally.
5. Verify the service's proven endpoint and exact operational public key. Deny,
   restart, substitute a device/endpoint, interrupt enrollment and crash/reload
   the renderer; stale proofs must not become current. Never enable input.
6. Record enrollment separately from WIN-P01. The current frozen native harness
   calls `publisher.share()` and waits for decoded capture before checking the
   endpoint. Full WIN-P01 consequently still needs qualified real capture.

No approved disposable service/device or test issuer was supplied for this run.
Service enrollment, real native consent and WIN-P01 are Not Verified. Production
issuer #47, TURN #48 and independent integrated review #33 remain separate gates.

## Verification record

Executed checks and final candidate/CI references are recorded below as they
finish. No physical input, clipboard, personal capture or installed upgrade is
executed by the no-authority process and isolated Electron fixtures.

Local candidate verification passed:

- `npm run check`: 226 application tests and syntax checks.
- `npm run test:assist-native`: formatting, strict all-target/all-feature Clippy,
  151 library plus 54 integration tests in each default/all-feature configuration.
- `npm run test:assist-process`: 33 real no-authority process groups per build
  configuration and the separate hidden-startup native-control fixture.
- `npm run package:win`: Windows x64 NSIS packaging. No installer was executed.
- `npm run verify:assist-package` and `npm run verify:update-package`: protected
  ASAR/native hashes, production trust/profile rejection and packaged source checks.
- Packaged `test:window-controls`, `test:assist-capture`, `test:assist-peer`:
  isolated Electron controls, permissions, native endpoint/crash plumbing, existing
  capture components and 14 fake-sink input-transport fixtures. These do not qualify
  real remote viewing or physical input.
- `npm audit --omit=dev --audit-level=high` and both locked `cargo audit --deny
  warnings` checks: no reported vulnerabilities.
- Explicit unsigned qualification directory build using a reserved `.invalid`
  origin and the existing canonical fixture's public issuer fingerprint. Real
  Electron loaded its ASAR and compiled helper for no-authority handshake/crash
  checks. No issuer HTTP, native consent, identity storage or service proof ran.

Independent source review found and corrected the compiled-pin confirmation
fallback, confirmed-record pin override, trailing-dot production-host alias and
port-zero validation mismatch. Negative tests cover those paths. The reviewer
cleared the inactive/qualification source scope, not production activation or
physical consent/input. The separate normal custody design review requires
monitor-owned spawning, one child terminal cleanup owner and reconciliation that
cannot issue duplicate mirror UPs; implementing those remains Windows work.

Earlier app tests initially failed because test module adapters had not included
new imports; they were updated and the full suite passed. The first real endpoint
document-replacement test retried an already revoked channel and hit its watchdog;
the corrected test verifies terminal refusal and joins that channel. An older
issuer test expected `issuer-unqualified` for a wrong explicit pin; the stricter
mandatory-pin behavior returns `issuer-changed`, and its expectation was corrected.

The implementation was pushed to main. Exact-source Windows build
[42986856090](https://github.com/cortezdemartino/palisade-windows/actions/runs/42986856090)
passed. Documentation checkpoint `d47647696dfb42d73ba8591c39ef5654480fefa5`
also passed [42987179543](https://github.com/cortezdemartino/palisade-windows/actions/runs/42987179543).

A final conformance correction widens the private version-offer representation
from u32 to u64, enforcing the canonical JavaScript safe-integer range rather
than narrowing a valid offer. It still negotiates only supported native version 1.
New boundary tests and actual monitor/coordinator hello with
`[Number.MAX_SAFE_INTEGER, 1]` pass. Full native suites now total 205 per
configuration; both 33-group process suites, final NSIS, packaged integrity and
actual isolated Electron/native crash checks were rerun successfully. Independent
source review cleared this correction within the same inactive scope.
This correction triggers a further exact-SHA Windows build; issue #1 records its
terminal outcome so a pending run is never represented as passed.
No release tag, signing workflow, publication, deployment or installation ran.
