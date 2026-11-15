# Assist master completion matrix

Campaign: M4G-ASSIST-FINAL-6. Windows baseline
`ca983d074738d74bbe3388fc70578813e8421297`; companion inspected read-only at
`74e8fa8525677f7f0cba8d3106f00a7e65f3c042`. Updated November 15, 2026.

**Engineering completion: Blocked. Full-feature release eligibility: Blocked.**
An implemented component, successful package or passing simulated test does not
establish a functioning accepted feature. No deployment, installation, production
activation or release is authorized by this campaign.

This is the authoritative feature inventory. Older dated checkpoint tables remain
historical evidence. Allowed classifications are Not Started, Implemented,
Integrated, Verified, Blocked, Not Verified and Approved. Verified below always
names its limited evidence class; no row is Approved for release.

## Functional inventory

| Feature | Windows/native implementation | Web/backend and UI | Automated evidence | Real-device acceptance / release | Remaining work and owner |
| --- | --- | --- | --- | --- | --- |
| Encrypted screen/window viewing | Integrated: owned picker, source binding and encrypted publication | Implemented | Verified: owned-window/synthetic DTLS and capture refusal tests | Not Verified: WIN-C01/C02 | Windows/operator: actual enrolled screen/window publication and source replacement |
| Mouse control | Integrated: consent, signed authority, verified certificate, owned executor | Implemented | Verified: signed fixtures, fake sink, no-authority process/custody tests | Not Verified: WIN-I01/I03/I04/I05 | Windows/operator: actual mapped movement/buttons/wheel/drag and refusal tests |
| Keyboard control | Integrated: grant-scoped keys/modifiers/shortcuts | Implemented | Verified: Profile 2/ownership/negative tests | Not Verified: WIN-I02/I05 | Windows/operator: actual layouts, holds and cleanup |
| Virtual keyboard/Unicode | Integrated: native Profile 2 decoder/executor | Implemented | Verified: protocol/primitive tests | Not Verified: WIN-I02 | Windows/operator: exact text and permitted shortcuts on target apps |
| Native permission prompts | Integrated: enrolled identity, guarded consent, per-request cancellation | Implemented | Verified: no-authority cancellation/provenance tests | Not Verified: WIN-C01/I04/L02/T03 | Windows/operator: physical Allow/Deny, source and stale-dialog checks |
| Emergency Stop | Integrated: independent native event, retained custody and cleanup | Implemented | Verified: real no-authority process/event and simulated executor tests | Not Verified: WIN-L01/L03/L05/L07 | Windows/operator: actual held-input Stop under blocked/saturated work |
| Fresh control reacquisition | Integrated: same gate/deadline, new signed generation, consent and peer | Blocked: stale peer loss can revoke successor | Verified: signed/cancellation/cohort/cleanup tests | Not Verified: WIN-L02/L06 | Companion: conditional revoke and incarnation retirement; joint acceptance |
| Seamless cooperative control | Blocked: no proven delivery-time host/helper ownership arbitration | Blocked: frozen v3 requires 2,000 ms idle; immediate handoff needs negotiated successor | Verified: old scheduler only in simulation | Blocked: WIN-Y01–Y04 plus new seamless cases | Native/security reviewers: supported safe mechanism; companion: separately negotiated successor after design; physical qualification |
| Clipboard text | Implemented: pure broker and resource bootstrap; live native path Not Started | Implemented | Verified: broker/protocol tests only | Not Verified: WIN-K01 | Windows: resource DTLS, consent, native clipboard adapter and acknowledgements |
| Clipboard PNG images | Not Started: live native decoding/Copy/Save | Implemented | Verified: frozen fixtures only; header checks are not safe decoding | Not Verified: WIN-K02 | Windows: bounded actual decode, protected UI, encrypted send/receive |
| Clipboard synchronization | Implemented: dormant broker; native integration Not Started | Implemented | Verified: broker/contract tests only | Not Verified: WIN-K01/K02 | Windows: directional native opt-in, observation, canonical rate/echo suppression, urgent disable |
| Single/multiple files | Implemented: guarded file primitives; live integration Not Started | Implemented | Verified: metadata/path/broker tests | Not Verified: WIN-F01/F02 | Windows: shared resource transport, per-file consent, private staging, chunks and whole-file integrity |
| File progress/cancel/retry | Implemented: dormant broker/extension validation | Implemented | Verified: component tests | Not Verified: WIN-F01/F02 | Windows: transfer-owned event lifetimes and reserved cancel capacity |
| File resume | Not Started: live authorized interrupted staging/resume | Implemented | Verified: frozen resume fixtures only | Not Verified: WIN-F03 | Windows: private ten-minute interrupted staging, fresh authority/peer/consent and verified offset/hash |
| Drag/drop | Not Started: native owned drop surface and guarded per-file selection | Implemented | Verified: frozen extension validation only | Not Verified: WIN-F02 | Windows: app-owned drop confirmation; never expose paths to hosted page |
| Pointer/annotations | Implemented: native capability refusal boundary; no OS input implied | Implemented | Verified: companion browser evidence at its recorded candidate, not this candidate | Not Verified: WIN-A01 | Joint operator: actual viewing/annotation regression on exact candidates |
| Multi-monitor | Integrated: native source/geometry components | Implemented | Verified: geometry/source tests | Not Verified: WIN-C02/I03 | Windows/operator: physical negative coordinates, mixed DPI, portrait and monitor removal |
| Session/resource recovery | Implemented: endpoint bootstrap, global lease ordering and fresh input generation; resource transport Not Started | Blocked: Stop Control currently invokes global native Stop | Verified: signed lease/reacquisition tests | Not Verified: WIN-L02/L06/F03 | Windows: independent resource liveness; companion: input-scoped Stop and peer lifetime fixes |
| Crash/lock/sleep recovery | Integrated: terminal fencing/custody/cleanup components | Implemented | Verified: no-authority subprocess crash/parent-loss tests | Not Verified: WIN-L03–L07 | Windows/operator: real input/resource cleanup and no implicit restoration |
| Update coexistence | Integrated: shared quit/update cleanup barriers; signed in-app updater preserved | Implemented | Verified: updater, lifecycle and packaged fixtures | Not Verified: WIN-U01 installed active-session upgrade | Windows/operator: approved disposable installed upgrade with actual owned cleanup |

## Cross-cutting gates

| Gate | Classification | Evidence or exact dependency | Owner |
| --- | --- | --- | --- |
| Frozen compatibility | Verified: source/fixture comparison | Six sources, nine fixture families and all 30 cases match inspected companion; manifest SHA-256 `f9ac9d835333e341907ae63f2d2a497ef4944607ea60fc344e81ad64115606b0` | Windows/companion |
| Resource-only authority bootstrap | Integrated | Same registry, gate, original deadline and endpoint-wide lease lane; grants no OS consent or input readiness | Windows |
| Independent resource/input transport liveness | Integrated: transport only | Native purpose/cancel witness and trusted bridge lifetime preserve shared association through input Stop; 18 synthetic/fake-sink DTLS fixtures pass. Resource actions and companion fixes remain unfinished | Windows/companion |
| Companion stale peer loss / Stop Control | Blocked | Exact reproducer and correction in [handoff](ASSIST-COMPANION-HANDOFF-PROMPT.md); still present at inspected SHA | Companion |
| Approved qualification environment | Blocked | Existing Windows #5 requires disposable device/accounts, approved installation and isolated service/issuer pin; none supplied | Owner/operator |
| Production issuer trust | Blocked | Dedicated issuer and independently verified release pin, project-palisade#47; test issuer is sufficient for isolated tests | Owner/operator |
| Relay-only connectivity | Integrated: native configuration; actual relay Not Verified | Purpose-signed endpoint-ice, private ordinary lane and native-only gateway config preserve relay-only policy; actual isolated TURN credentials/relay evidence still required, project-palisade#48 | Windows/operator |
| Firefox qualification | Not Verified | Existing host limitations and project-palisade#49; no weakened browser security | Companion/operator |
| Integrated independent review | Not Verified | Focused component reviews do not satisfy candidate-specific full-chain review, project-palisade#33 | Independent reviewer |
| Candidate CI/package | Verified: baseline only | ca983d0 [Windows CI](https://github.com/cortezdemartino/palisade-windows/actions/runs/43745042507); unpublished NSIS built, not installed | Windows |
| Owner exact-candidate release approval | Not Verified | No full-feature approval and no release authorized by this brief | Owner |

## Next executable work

1. Resource-only/shared DTLS now connects to the retained native gate with separate
   readiness and cancellation. Finish permission-scoped payload dispatch and outbound
   framing before claiming working resource transport or enabling an OS action.
2. Complete protected resource consent, text/PNG adapters, transfer staging/chunks,
   extension progress/cancel/sync and owned drop UI using the existing brokers.
3. Native authenticated ICE is integrated; actual relay testing requires the
   [isolated setup prerequisites](ASSIST-QUALIFICATION-SETUP.md). Verify exact-candidate tests, packaging and CI; update this matrix with narrow
   evidence classifications after each integrated slice.
4. Run docs/54 against a real approved endpoint when Windows #5 prerequisites exist.
   Preserve every unexecuted case as Not Verified. The full product remains Blocked
   while seamless cooperative input lacks a proven safe mechanism.

No fixed-delay change to frozen v3, synthetic input record, mock endpoint or passing
unit suite satisfies the new seamless handoff requirement. A driver is a candidate
architecture only; this campaign does not authorize installing privileged code.
