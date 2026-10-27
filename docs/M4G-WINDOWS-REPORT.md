# M4G-WINDOWS-FULL implementation report

Checkpoint: October 26, 2026. **Full Assist: Blocked.** Windows has reviewed, tested inactive components; the product is not release-ready. Companion source remains read-only.

Starting Windows revision for this continuation: ffff0624b4b779814605530fd432e3143d19906d. Earlier component work began at c76d3370a770e811e4997d61cab738a9a9a90d3e. Exact final candidate and its CI URL are recorded in the pushed commit/issue evidence. Companion inspected: c0778935a6a08303bf7a9d7de8f2fa75aa8ef536.

| Requested area | Classification | Evidence or remaining work |
| --- | --- | --- |
| Canonical contract | Implemented, inactive | docs/48 stream/group control v2 source/schema/fixtures pinned; legacy v1 unchanged |
| Endpoint architecture | Experimental | Per-user Rust guardian, private pipes, child input, native consent and local stop; no privileged service |
| Native modules | Experimental | stream, executor, input, runtime, consent, lease, identity, issuer, files, image, parent and lifecycle |
| Electron description bridge | Implemented | Original three members/seven fields; unavailable/absent production report |
| Native page bridge / remote gateway | Blocked | Final canonical answer/cancel/status/lifecycle and certificate binding required, then Windows implementation/wiring |
| Screen/window capture | Experimental, inactive | Existing picker, separate provider, local owned DTLS video, actual certificate/socket checks and selected-window stop |
| Cross-client encrypted viewing | Not Verified | No MatrixRTC publication adapter or live session executed |
| Mouse | Experimental | Bounded coordinates/buttons/scroll, native source checks and fake-sink/process tests; real input not exercised |
| Keyboard / shortcuts | Experimental | Physical scan-code mapping, permitted bounded shortcuts and owned modifier cleanup |
| Virtual keyboard / toolbar | Blocked | Canonical web UI and native adapter integration |
| Multiple monitors | Experimental / Not Verified | Negative origin/rotation coordinate tests and native physical monitor bounds; hardware acceptance pending |
| Clipboard | Experimental | Separate directions, local action, default Off and sync agreement; OS/peer adapter not wired |
| Files | Experimental, inactive | Native session-owned publisher requests, exact offer binding, directional cancellation and atomic shared quota; protected NTFS tests pass; IPC/transport and physical confirmation pending |
| Transfer encryption | Blocked / Not Verified | Reuse canonical encrypted data peer; local file hash tests do not prove confidentiality |
| Consent | Experimental | Native helper/device/capability/expiry display, default Deny, post-decision validation and signed acknowledgement |
| Emergency stop | Experimental / Not Verified | Native stop message-pump and visibility fixtures; actual shortcut/input cleanup acceptance pending |
| Expiration and revocation | Implemented, inactive | Immutable two-hour ceiling, 20s signed lease, five-second endpoint polling, fail-closed clocks and terminal grants |
| Input suspension | Implemented, inactive | Independent input epoch; release/fence while resources retain separate authority; full renewal cannot restore input |
| One-controller ownership | Implemented, inactive | Shared named-event lifetime across guardian/child; real two-process crash exclusion fixture |
| Windows lifecycle | Experimental / Not Verified | Default desktop/SID/UIPI checks and release-only retry; physical lock/sleep/secure desktop tests pending |
| Cross-client interoperability | Not Verified | No installed endpoint or full session tested |
| Companion requirements | Blocked | 97b2a4f addresses the two service defects in source; official verification, finalized native/capture adapter and enrollment trust remain |
| Performance | Not Verified | No invented latency, throughput or resource results |
| Security review | Implemented for components | Capture cancellation/liveness and input ownership/polling findings resolved; product audit/acceptance pending |
| Unit tests | Implemented | Current executed counts in TEST-RESULTS.md |
| Native Windows tests | Implemented for fixtures | Owned video/windows, DPAPI, NTFS and subprocess/lifetime tests only |
| IPC/negative tests | Implemented for components | Provenance, frames, replay, tampering, deadlines, cancellation, frozen peers, ownership |
| Browser/cross-client tests | Not Verified | Hosted denial fixtures run; live peer matrix remains pending |
| Packaging | Implemented when checks pass | Default-sensitive-features-off x64 NSIS; exact result recorded in TEST-RESULTS.md |
| Signing/update boundary | Implemented | Existing trusted updater/fuses/identity/partition preserved; no release signed this checkpoint |
| Documentation | Implemented | Integration, capture, native architecture, threat model, user/operations/settings and companion prompt |
| Issue tracking | Implemented via owner issues | Windows #1/#3/#4/#5, project #18/#33; exact checkpoint evidence posted after push |
| Open owner interventions | Blocked | Companion changes, disposable-device acceptance and separate installation/activation/release approval |
| Exact-SHA CI | Not Verified until hosted run | Previous ffff062 run passed; current push must establish its own result |
| Deployment | Blocked | No installer executed, production feature enabled or release published |
| Limitations | Blocked / Not Verified | Images, transfer resume and unrestricted drag/drop unsupported; no unattended access |
| Next integration | Blocked | Companion handoff first, then Windows remote gateway/native IPC/resource/capture wiring and real acceptance |

The remaining work includes Windows integration, not only web code. A production endpoint cannot be completed by accepting page-provided consent or fingerprints. Generic hosted capture remains denied. The isolated local video hop does not replace MatrixRTC E2EE.

Owner architecture approval is already recorded under project-palisade#33. Independent component review permits committing dormant code, not installation or activation. Windows#5 requires disposable devices/accounts and real emergency-stop acceptance. No input was injected into the owner's personal desktop. Window-bounds persistence remains queued after Assist testing readiness.

Follow [the release plan](ASSIST-RELEASE-PLAN.md). Use [the companion prompt](ASSIST-COMPANION-HANDOFF-PROMPT.md) for the read-only dependency. See [integration](ASSIST-INTEGRATION.md), [capture](ASSIST-CAPTURE.md), [native components](ASSIST-NATIVE-GUARDIAN.md), [threat model](ASSIST-THREAT-MODEL.md) and [executed tests](TEST-RESULTS.md).
