# Assist continuation checkpoint

## Monitor custody continuation

`718348fd7dc4fbb907da7bda897fe09da0500c3a` pushed the custody milestone.
Normal ProcessInput is now being corrected to close/poll instead of kill: retain
startup failures, distinguish admission fencing from actual reaping, and refuse
mirror UP until confirmed exit. This does not attach the normal child to custody.
[Fence evidence](evidence/2026-11-09-nonkilling-input-fence.md). The next normal
channel must retain monitor/coordinator witnesses and child/mirror receipts; never
replace that missing proof with an exit status or the probe's synthetic mirror.

The monitor custody model and real no-input process adapter now retain executor
ownership through ARM ambiguity, bootstrap/query errors, coordinator death and
blocked ordinary output. Dedicated child receipts, exact retained process handles
and settled mirror evidence are separate requirements. Probe receipts cannot
satisfy native cleanup. [Evidence and next integration](evidence/2026-11-09-monitor-custody.md).
The normal input child/mirror wire, sealed launch bootstrap and persistent runtime
owner remain unfinished; do not widen private v1/v2 scope or advertise localYield.
The queued COOP-1 attachment repeats the earlier contract. Preserve the newer frozen
`01ae585` manifest v3, all nine fixture families and all 30 acceptance cases.

## Independent grant admission continuation

Current-view admission now uses the immutable constructor generation floor rather
than iteration-dependent highest generation. Resource-only binding uses canonical
null endpoint semantics while preserving local enrollment identity. A sticky
terminal-input latch refuses unseen input capabilities after Stop; it has no reset.
[Evidence](evidence/2026-11-09-independent-grant-admission.md). This is not fresh
reacquisition. Continue positive monitor-owned executor receipts, then the sealed
launch bootstrap and persistent session owner; do not widen private IPC authority
from process-exit proof or descriptive service facts.

## Frozen service response continuation

The native lease-view parser now accepts the exact frozen service fields
`streamTrackName`, `approved` and `inputRequest`, with strict metadata validation
and unchanged signed authority/consent boundaries. The earlier legacy-shaped
regression missed this incompatibility. [Evidence and source hashes](evidence/2026-11-09-frozen-service-view.md).
The main integration guide now has an authoritative current ledger above its old
checkpoint notes. Continue positive monitor cleanup receipts and the persistent
launch/session owner; capture corroboration and full runtime/resource integration
remain unfinished. Neither service facts nor source names grant native authority.

## Direct monitored cancellation continuation

Session isolation `5d95cdadaad87f4effb99ff0e653b3a903ce93e2` passed full exact-SHA
CI [42679846356](https://github.com/cortezdemartino/palisade-windows/actions/runs/42679846356).
The coordinator now checks exact wait-only monitor/event handles directly and
retains terminal Shutdown ingress after the event is signaled. Stop controls start
before the coordinator. [Evidence](evidence/2026-11-09-direct-native-stop.md).
No authority-bootstrap proof, runtime or wider cleanup scope is exposed. Continue
positive cleanup receipts and the witness-preserving launch/session registry.

## Per-session cancellation continuation

The observer correction `11f650890192f7ec79035c98424adbbe2e01e3ad` passed exact-SHA
CI [42677575263](https://github.com/cortezdemartino/palisade-windows/actions/runs/42677575263).
Native runtime sessions now have independent terminal cancellation under one global
emergency stop. Gate admissions and file dialogs read both scopes directly; OS
security failures still stop the launch. [Evidence and remaining integration](evidence/2026-11-09-session-cancellation.md).
Continue the launch-owned signer/session registry and positive descendant cleanup.
This does not complete or activate Assist.

## Cold observer startup correction

The endpoint-key checkpoint `e4e5332` failed exact-SHA CI 42676519053 in the
parent-loss fixture before process-handle retention. The observer now starts
before the six-second event-only holder; native timeouts, the four-second
post-parent-death assertion and required exit codes are unchanged. See
[evidence](evidence/2026-11-09-process-observer-startup.md). Continue native
launch-owned registry/runtime and positive cleanup work after checking the
successor CI; full Assist is unfinished.

## Operational endpoint signer continuation

Local executor registration at `3fc0e1406b2528d09ad16c5d93086bf6ab5c06ed` passed exact-SHA CI [42625418856](https://github.com/cortezdemartino/palisade-windows/actions/runs/42625418856). A separate operational endpoint key type now protects the native API from accidentally reusing the preserved DPAPI installation signer after restart. See [evidence and remaining owner/tombstone work](evidence/2026-11-09-operational-endpoint-key.md). The frozen manifest v3 is now pinned from `01ae585`, with all 30 acceptance cases and unchanged protocol/fixture bytes. The real authority owner must still retain one key per launch, perform fresh canonical enrollment and prevent in-launch session resets. Live native runtime, fresh acquisition and resource integration remain unfinished.

## Local executor registration continuation

The parent-loss correction at `5046a0d0d95ddae6da2f31bae580c4542a84d863` passed exact-SHA Windows CI [42620269751](https://github.com/cortezdemartino/palisade-windows/actions/runs/42620269751). The next dormant slice binds local execution permits and terminal cleanup to one exact gate/executor reservation, with one-shot cleanup requirements and receipts. See [evidence and next integration constraints](evidence/2026-11-09-executor-registration.md). It does not yet implement same-capability fresh-grant admission, new peer acquisition or crash-surviving monitor receipts. Do not reset terminal atomics or report full Assist ready.

## November 9 parent-loss correction

The cooperative scheduler checkpoint is pushed as `ab94c6fda59d37e94c47ae18cbd21d4f519c6372`. Its first exact-SHA CI failed the parent-loss process case. Investigation found libuv kill-on-parent-exit launch policy and broken-stderr panic defects. The new launchers keep native cleanup alive with referenced detached processes; the enhanced test retains all three original process handles and tests clean/forced fixture-parent death with the original four-second deadline. See [evidence](evidence/2026-11-09-native-parent-loss.md). Do not count the failed run as a CI pass.

Companion main has advanced read-only to `01ae58565dd29dce692bace6d09834c5efdbff69` (contract freeze, docs/53). All consumed protocol sources and fixtures remain byte-identical to the pinned `e45cc8f` contract. Its manifest now adds acceptance evidence/metadata, without changing the wire. Do not invent a temporary protocol or ask the web implementation to recreate these contracts.

## M4G-WINDOWS-FINAL execution

Started from `d5787528d895202b3b9d41bfd9ce10a3212c4e57`. The direct process-ownership milestone is committed as `6c203cc1ca1ab9a0aa50297be73f37079fc7f7e6` and its exact-SHA Windows CI [42563399929](https://github.com/cortezdemartino/palisade-windows/actions/runs/42563399929) passed, including packaging and packaged checks. Companion main has advanced to `e45cc8f20b3060e8231ecafc29c2417ee6f42040`; the canonical cooperative contract has arrived and was independently verified read-only before updating Windows pins.

The queued M4G-WINDOWS-COOP-1 work now includes an inactive native scheduler, atomic execution fence, original-ingress readiness barrier, semantic signed-authority snapshot and nonterminal cleanup hooks in the existing Executor. All ten canonical endpoint scenarios and three helper scenarios are executed in simulation. Existing Windows adapters remain unqualified, `localYield` stays unadvertised, and physical observation/delivery arbitration/live status remain unfinished. See [implementation](ASSIST-COOPERATIVE-INPUT.md) and [evidence](evidence/2026-11-08-cooperative-core.md). Continue physical arbitration alongside positive native cleanup/runtime integration; this is not a completion report.

The first ownership milestone adds actual atomic non-killing job assignment, strict handle inheritance and a direct application-owned monitor. Private version2 negotiates only `channel-and-process-exit`, with native input false; version1 remains unchanged. A pre-launch scope guard and coordinator-death handling signal stop before descendant waits. Real no-input process tests cover delayed survivors, stop-event-only survivors, saturation, bad frames/images, EOF, parent death, blocked output and monitor crash. [Evidence](evidence/2026-11-08-process-monitor.md).

Next: register positive executor-generation cleanup receipts and direct authenticated receipt ownership before input admission, then attach the real persistent enrollment/session runtime, sensitive native UI guard, source/transport signing and extension resources. Process accounting is not input cleanup proof. Keep unknown cleanup failed, no killing/bypass, no production activation or release. The earlier all-feature test fail-fast is documented and not claimed fixed. Complete available implementation beyond this component milestone; it does not finish the assignment.

## M4G-WEB-6 integration after desktop v0.3.3

Desktop v0.3.3 is published at `5ebca7f4f52f62e0460da47f768323d19a1c4e60`; exact CI/release and six public assets passed signature/hash/discovery verification. The following handoff is now being integrated. Native release authority is a component checkpoint only; see [evidence](evidence/2026-11-08-release-authority.md). Existing v1 sources and the seven earlier fixtures remain unchanged. The owner additionally requires cooperative host/helper input with local priority. Current local-activity hard stop remains unchanged until a canonical nonterminal local-yield contract and native integration are implemented; use the updated [Claude prompt](ASSIST-COMPANION-HANDOFF-PROMPT.md).

Owner supplied the M4G-WEB-6 handoff for companion commit `5b82bc90e4997f35dd18273427c258fe54eefb95`. The desktop maintenance release is complete. Companion inspection remains read-only; the integration does not activate Assist.

1. Read companion `docs/52-ASSIST-WINDOWS-ACCEPTANCE.md`, `docs/51-ASSIST-FEATURE-COMPLETION.md`, `docs/assist-interop/manifest.json` and `docs/evidence/m4g-web-6-integration-acceptance.md`. Verify the source blobs and hashes independently before re-pinning.
2. The handoff reports a deliberate v2-only change to `protocol.ts`, blob `9d499137d469280c8798e72bed4ef88b56a318c3`; v1 and the seven existing fixtures/extension source remain unchanged. New `docs/assist-control/v2-release/fixtures.json` SHA-256: `1bc0231199e1c342fa885f3dd30bbe84e3db2b671c7b14c083067143bed73cc6`. Interop manifest SHA-256: `4ab9e541ac052fac32ca340a0585848e42cbc0388fde0530c493319d1ce17e03`.
3. Reconcile native `stream.rs` and all affected gateway/executor checks: v2 release-all may use an active mouse or keyboard grant, but releases only input held by that sender under that grant. Never add keyboard permission to a mouse-only session; preserve wrong-session, revoked/resource-grant and unowned-release refusal/no-op semantics and independent endpoint teardown.
4. Keep the separate new-grant/cleanup/consent/peer requirements, extension v2 dispatch and guarded native UI work in the existing full Assist plan. Run canonical fixtures and targeted negatives before broader checks. WIN-P01, WIN-I05 and WIN-L02 require approved disposable-device qualification; do not run remote-input acceptance on the owner's desktop.

Reported companion results are 1019 web and 280 service units plus two full Chromium runs (22 passed/one Firefox-only skip each). These results have not been rerun here. Firefox, working TURN and the real Windows endpoint remain Not Verified in that handoff. No deployment or enablement is inferred. Existing issue references: project#18 comment6725632964, project#33 comment6725634554, Windows#4 comment6725638644. Architecture approval is already recorded; retain the existing installation/activation/physical/release gates.

## November 8 Profile 2 continuation

Companion refreshed read-only at `ae3542fc0adb9d7f19b3e9a8521fb039555ee7f6`; Windows began at `87daf784daa48d1ce3b64460dcf67fa27cb71a93`. Docs/51 now defines the full added web contracts. Native Profile 2 parsing, key/button/Unicode execution and authoritative age/queue admission are implemented as an inactive component. [Scope and evidence](evidence/2026-11-08-profile2-native.md). Full live coordinator, crash-surviving cleanup, protected approval UI, fresh control generations and extension v2 resources remain unfinished. Do not ask Claude to recreate the new contracts. The mouse-only release-all defect is tracked in Windows #4.

Verified baseline: Windows 9f529087653d4b0259d4c0749bc9c8dd7f692978, exact-SHA CI40787475543 passed. This continuation adds dormant atomic revocation, retained cleanup and independent consent; its final pushed SHA/CI is in Git history and Windows#1. Companion refreshed read-only at 82ac7de4ec2bbd5cc2fd48f3d15dbde6ea5fd375.

## Current result

W1 shared Quit/update-manager cleanup ownership is committed and green. W2's non-authorizing channel foundation is implemented: separate --live/--live-probe, strict private framing, bounded queues, independent reader/watchdog, safety shutdown, lifecycle-before-spawn, exact launch-nonce/channel-only completion AND actual process exit. No runtime/session/identity/input/resource command is exposed. It never kills helpers; an unproven crash retains failed ownership. This does not complete W2 or full Assist.

Native internal APIs now separate prepared atomic RuntimeRevoker/FileRevoker from blocking retained cleanup. Prompt/poller/input/file joins report sticky failures; failed owners remain retained, and panic/destructor/payload failure does not masquerade as success. Native prompts cancel before gate waits and track global/input-only flags. Exact own-grant consent survives unrelated input suspension. Files close without blocking registry/I/O locks; wait_cleanup proves both stores/selection/watcher completion. The release helper uses Rust unwinding and a fixed synthetic cleanup probe. None of this attaches the runtime to the channel or provides full-lifetime sensitive-UI guarding.

Local:168 app tests;85 native tests per configuration;17 real subprocess groups per configuration plus visibility; x64 NSIS/package integrity and actual packaged cleanup probe; packaged desktop/capture/14 fake-sink peer cases; audits zero vulnerabilities. Installer80607651456da6c5fd3d6858282685cad48a9b40414361cd88b43232876abc48, unsigned/uninstalled. Helper264a40d252544583800baf4360fcec0bebf94c68ecac2ba65760b30aeb15596a matches built/packaged bytes. Initial EBUSY/unread timing failures and corrected reruns are recorded. Independent review cleared this inactive slice. See TEST-RESULTS.md and ASSIST-LIVE-CHANNEL.md.

## Next implementation

Read ASSIST-RELEASE-PLAN.md. Finish W2 authenticated descendant ownership and live runtime integration using the implemented atomic revokers and retained join APIs. No gate lock, prompt drop, file sweep, HTTPS or join on urgent threads. Preserve the exact-grant resource consent fix and sticky worker failures. Do not treat native Drop/is_finished, channel-only proof or a direct guardian exit as active-session completion. Release abort/foreign/FFI and combined host failures remain outside Rust unwinding recovery.

Before authority, retain authenticated ownership through ALL cleanup descendants. Direct guardian exit/channel-only proof is insufficient. Consider a non-killing/no-breakaway job with exact retained handle/assignment before authority; query actual ActiveProcesses==0 after closing admission, never rely solely on job signaling/completion notifications or process-name polling. Close the crash-before-child-report race. Current channel deliberately refuses broader cleanup scope. NativeStop/session flags must be connected to the same revocation owner.

Keep SessionGate/LeasePoller as one registry; grant offers do not recreate runtime or restore revoked grants. Then W3 source/consent/owned-answer signing and W4 fresh input generations/readiness. Current monitor geometry cannot silently stand in for window-only input; keyboard desktop scope needs truthful consent. Transport setup precedes DTLS: require current source/enrollment/consent/covering lease without temporarily setting peer_verified. Await native observed-peer readiness/idle warmup before packets.

Sensitive UI must revoke/fence/reap/release before approvals, block new controllers/ACK/signing/warmup throughout, and require fresh input afterward. Cover capture/enrollment/settings/clipboard/files/save. Stop/Deny remain usable. Keep independent resources. App-owned drops need per-file local confirmation before any content read; hosted page must see no file objects/bytes during any drop phase. isTrusted and remote/cleanup-only UP events are not host consent.

## Companion and gates

Companion remains read-only. Docs/50 at82ac7de now supplies initial text/file channels, local actions, browser-owned helper authority, relay route, track names, safety capacity and cleanup events. Integrate these; do not ask Claude to recreate them. Its images/sync/resume/drop remain unsupported and open requirements. Updated ASSIST-COMPANION-HANDOFF-PROMPT.md covers remaining full features, keyboard/annotations/reacquisition, stale SCTP input, relay and joint acceptance.

Official docs/42 uses equivalent exact-candidate LOCAL full verification while companion Actions are owner-disabled. CI absence alone is not a blocker. Windows CI remains enabled and requires the exact pushed SHA. Production issuer is project#47; disposable-device/native/joint acceptance is Windows#5, with no test-device answer received.

Architecture and reviewer authorization already exist. Installation, native activation and real input on approved disposable devices require separate approval; never use the personal desktop. Publication/production remain separately gated. No private production keys, installation or activation occurred. Keep this existing main checkout, profile/session/updater identity and stopped v0.3.0 tag. Window persistence stays queued after Assist readiness.
