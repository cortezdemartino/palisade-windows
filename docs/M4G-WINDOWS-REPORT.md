# M4G-WINDOWS-FULL implementation report

Checkpoint: October 28, 2026. **Full Assist: Blocked.** Windows has reviewed, tested inactive components; the product is not release-ready. Companion source remains read-only.

Current continuation starts at `7afbc26be921f12ec2b722b4948a2025d7434453` (Windows CI 40610053435 passed). The new [private native channel](ASSIST-LIVE-CHANNEL.md) implements bounded typed framing, independent urgent cancellation/liveness and lifecycle registration before spawn. Matching channel-only proof plus actual clean exit is required. It has no native runtime or resource authority. Runtime cleanup, crash-surviving descendants, guarded consent and source/signing/peer/resource wiring remain Windows work. Latest companion inspected: `82ac7de4ec2bbd5cc2fd48f3d15dbde6ea5fd375`; docs/50's initial text/file/relay/safety/cleanup handoff is now available for integration. Images, sync, resume and drag/drop remain explicitly unsupported there and open in the complete feature ledger. Current executed evidence is in TEST-RESULTS.md.

Starting Windows revision for this bridge continuation: a7d381b2b92d0bf7f23c9d4d81f4c72e6cdfd1da. Earlier component work began at c76d3370a770e811e4997d61cab738a9a9a90d3e. Exact final candidate and its CI URL are recorded in the pushed commit/issue evidence. Companion inspected read-only: ae89e414ec71a193094ef7cbee5ad678418ff6f6; canonical native v1 validator is unchanged since 8f9c2a4.

| Requested area | Classification | Evidence or remaining work |
| --- | --- | --- |
| Canonical contract | Implemented, inactive | docs/48 stream/group control v2 source/schema/fixtures pinned; legacy v1 unchanged |
| Endpoint architecture | Experimental | Per-user Rust guardian, private pipes, child input, native consent and local stop; no privileged service |
| Native modules | Experimental | stream, executor, input, runtime, enrollment, consent, lease, identity, issuer, files, image, parent and lifecycle |
| Electron description bridge | Implemented | Original three members/seven fields; unavailable/absent production report |
| Native page bridge / remote gateway | Implemented, inactive / Experimental | Two-member bridge and app-owned data-only peer; actual helper certificate, native-admission boundary, bounded traffic and teardown tested with a fake native sink; live coordinator/signing/IPC remains unfinished |
| Screen/window capture | Experimental, inactive | Canonical native bridge/source operation/one-time tentative SID path plus existing picker/provider; owned-window pixels/source-stop and stale provider callback tests pass |
| Cross-client encrypted viewing | Not Verified | Companion publication adapter exists and is mock-browser tested; Windows capture binding and real cross-client session remain |
| Mouse | Experimental | Bounded coordinates/buttons/scroll, native source checks and fake-sink/process tests; real input not exercised |
| Keyboard / shortcuts | Experimental | Physical scan-code mapping, permitted bounded shortcuts and owned modifier cleanup |
| Virtual keyboard / toolbar | Experimental / Blocked | Companion web controls implemented; Windows endpoint integration and acceptance remain |
| Multiple monitors | Experimental / Not Verified | Negative origin/rotation coordinate tests and native physical monitor bounds; hardware acceptance pending |
| Clipboard | Experimental | Separate directions, local action, default Off and sync agreement; OS/peer adapter not wired |
| Files | Experimental, inactive | Native session-owned publisher requests, exact offer binding, directional cancellation and atomic shared quota; protected NTFS tests pass; IPC/transport and physical confirmation pending |
| Transfer encryption | Blocked / Not Verified | Reuse canonical encrypted data peer; local file hash tests do not prove confidentiality |
| Consent | Experimental | Separate private enrollment tuple and native capability consent; own-key/source expectations, cancellation/revocation and labelled proof/ack; real dialog acceptance pending |
| Emergency stop | Experimental / Not Verified | Native stop message-pump and visibility fixtures; actual shortcut/input cleanup acceptance pending |
| Expiration and revocation | Implemented, inactive | Immutable two-hour ceiling, 20s signed lease, five-second endpoint polling, fail-closed clocks and terminal grants |
| Input suspension | Implemented, inactive | Independent input epoch; release/fence while resources retain separate authority; full renewal cannot restore input |
| One-controller ownership | Implemented, inactive | Shared named-event lifetime across guardian/child; real two-process crash exclusion fixture |
| Windows lifecycle | Experimental / Not Verified | Default desktop/SID/UIPI checks and release-only retry; physical lock/sleep/secure desktop tests pending |
| Application quit/update ownership | Implemented for application | Shared barrier before exit/manager copy/spawn, pending status and retry; real Electron fixture uses a fake holder; native endpoint registration/physical cleanup acceptance pending |
| Native revocation/retained cleanup | Implemented, inactive | Atomic revokers, separate joins, retained/sticky failed owners, exact-grant consent, two-store/watcher completion and actual release panic probe; live scope/descendant/physical proof pending |
| Cross-client interoperability | Not Verified | No installed endpoint or full session tested |
| Companion requirements | Blocked | Initial docs/50 resource/relay/safety/cleanup contract available; full-feature image/sync/resume/drop, keyboard/annotation/reacquisition reconciliation, real relay/joint evidence and production issuer remain; docs/42 owner-disabled Actions exception applies |
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
| Exact-SHA Windows CI | Not Verified for current push until hosted run | Baseline 7fcae47 passed run 40602184816; the current plan/lifecycle push must establish its own result; historical 2f250f6 capture run remains failed |
| Deployment | Blocked | No installer executed, production feature enabled or release published |
| Limitations | Blocked / Not Verified | Images, transfer resume and unrestricted drag/drop unsupported; no unattended access |
| Next integration | Blocked | Live coordinator, remote gateway/native IPC/capture wiring, canonical resource follow-up and real acceptance |

The remaining work includes Windows integration, not only web code. A production endpoint cannot be completed by accepting page-provided consent or fingerprints. Generic hosted capture remains denied. The isolated local video hop does not replace MatrixRTC E2EE.

Owner architecture approval is already recorded under project-palisade#33. Independent component review permits committing dormant code, not installation or activation. Windows#5 requires disposable devices/accounts and real emergency-stop acceptance. No input was injected into the owner's personal desktop. Window-bounds persistence remains queued after Assist testing readiness.

Follow [the release plan](ASSIST-RELEASE-PLAN.md). Use [the companion prompt](ASSIST-COMPANION-HANDOFF-PROMPT.md) for the read-only dependency. See [integration](ASSIST-INTEGRATION.md), [capture](ASSIST-CAPTURE.md), [native components](ASSIST-NATIVE-GUARDIAN.md), [threat model](ASSIST-THREAT-MODEL.md) and [executed tests](TEST-RESULTS.md).
