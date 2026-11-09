# Cooperative input contract and native scheduling checkpoint

Scope: inactive component milestone within M4G-WINDOWS-FINAL and the queued
M4G-WINDOWS-COOP-1 phase. This does not complete either assignment.

## Revisions and pins

- Original Windows baseline: `d5787528d895202b3b9d41bfd9ce10a3212c4e57`.
- This slice begins after `6c203cc1ca1ab9a0aa50297be73f37079fc7f7e6`.
- Companion inspected read-only: `e45cc8f20b3060e8231ecafc29c2417ee6f42040`.
- Exact prior-milestone Windows CI: [42563399929](https://github.com/cortezdemartino/palisade-windows/actions/runs/42563399929), passed including installer and packaged checks.

| Canonical source | Git blob |
| --- | --- |
| `control/protocol.ts` | `29bfd8d51d8cf423932379add25b83ae06334d8c` |
| `control/cooperative.ts` | `caaadb956475a91c2b7e1776651d7c7a8a88f873` |
| `native/extension.ts` | `005ac6ce6ca4b7c4fc27bcc32aa6d9d5f7ad03e1` |

Yield fixture SHA-256:
`5e678a011977aa063bba30200234ae9bb0fd971196daf198143e208155b74e89`.
Interop manifest SHA-256:
`345ce5758b965df67ada0544878c85aab1d922620a7d624cb82405afcdc42ac4`.

Both SHA-256 values and every Git blob were calculated from `git show` bytes before
updating the Windows pins. Every earlier canonical fixture was independently checked
against the new companion manifest and preserved. Provenance and hashes are enforced
by automated tests, using normalized LF source bytes on Windows.

## Implementation and evidence classification

| Component | Classification | Evidence |
| --- | --- | --- |
| Exact canonical messages, endpoint/follower behavior and ext v3 negotiation | Implemented / Verified in simulation | Pinned TypeScript code and fixtures executed with Node 22 |
| Native scheduler, separate execution fence, readiness ingress barrier | Implemented / Verified in simulation | Rust scheduler and existing Ledger, all ten endpoint sequences |
| Signed gate barrier validation and semantic authority snapshot | Implemented / Verified in simulation | Real Ed25519 fixture authority; fake OS delivery |
| Nonterminal executor cleanup and mirror retention | Implemented / Verified in simulation | Existing Executor and grant ledgers; positive fake acknowledgements |
| Physical observation and delivery-time overlap arbitration | Planned / Not Verified | No physical events or input were injected |
| Real native cooperative runtime, enrollment declaration and UI | Planned / Not Verified | Existing adapters deliberately do not qualify or advertise localYield |
| WIN-Y01 through WIN-Y04 on physical devices | Not Verified | Approved disposable-device access is still required |

The native implementation rechecks authority at every barrier/admission instead of
relying solely on the reference caller's periodic ticks. It preserves original
barrier ingress, so a delayed early barrier cannot become current after readiness.
It also preserves release-like local-queue handling for a valid later barrier.

Independent review identified and corrected:

- Terminal cleanup/Drop could miss a failed mirror update after a successful handover
  left the local ledger empty. Final mirror acknowledgement is now mandatory and
  retained even for an initially terminal release.
- Interrupted cooperative dispatch must latch cleanup pending even without a final
  held key, and only native admission interruption can take the nonterminal path.
- A foreign observation token could mark uncertainty without fencing the active
  scheduler. Invalid provenance now ends admission and cannot be resumed by a barrier.
- A processed native observation must not acknowledge a later queued physical hold.
  Each observation retains its original atomic generation and exact fence identity.

First focused test failures were harness errors: the authorization fixture uses
`authorized`, terminal spelling includes `local-stop`, a helper-originated status is
`not-granted`, and reused release fixtures have their own sequence/timestamp/button
values. The harness was corrected to follow those exact fixtures. No fixture bytes
or production security checks were relaxed to make tests pass.

## Candidate checks

- `npm run check`: **202 passed**, zero failed/skipped, syntax checks passed.
- `npm run test:assist-native`: formatting and strict all-target/all-feature Clippy passed;
  **120 native tests per default/all-feature configuration** (77 library, 15 executor,
  4 input-model, 5 Profile 2, 12 release/ownership/cooperative integration, 7 stream authority).
- Independent reviewer: **41 Node contract tests passed**; source review of this dormant
  boundary cleared after corrections. The reviewer did not execute native/physical tests.
- `npm run build:assist` and `npm run test:assist-process`: **25 actual no-input process groups per default/all-feature configuration**, plus hidden-startup control fixture, passed.
- `npm run package:win` and `npm run verify:assist-package`: passed; unsigned NSIS installer was built but not executed or published. Native sources, tests and private material excluded from the application archive; packaged helper integrity and fixed no-authority cleanup probe passed.
- Packaged `test:window-controls`: passed, including 30 permission cases and native registration fixture. Deliberately rejected renderer IPC appears as expected negative-test diagnostics.
- Packaged `test:assist-capture`: all five component groups and 12 local-video negative/lifecycle cases passed. These are owned local windows/synthetic video, not production cross-client Assist acceptance.
- Packaged `test:assist-peer`: **14 actual-DTLS / fake-input transport cases passed**.
- Production npm audit and locked cargo audit: no vulnerabilities reported.
- Installer SHA-256: `6b9a6afd96df844a44f38f8692badf9a277fabfd873d151556fae155ac58c174`.
- Built and packaged native helper SHA-256: `986cee10d4ce7bb22971b57562ed666b3dcce0980020a2a2550f633987c16b89`.
- Exact pushed-SHA Windows CI will be linked in the issue record. These local development bytes still have maintenance version 0.3.3; they are not the published v0.3.3 artifacts and must not replace them.

 The historical unexplained
native fail-fast from the previous slice remains documented in the
[process-monitor evidence](2026-11-08-process-monitor.md); successful reruns do not
establish its cause or resolution.

## Remaining work

Continue the actual native runtime and monitor-owned positive executor receipts,
physical arbitration, consent/capture and extension resources. Full Assist is not
waiting solely on web changes. No installer was executed, no private signing material
was used, no production switches were changed and no release was published.

The [cooperative implementation guide](../ASSIST-COOPERATIVE-INPUT.md) lists the exact
native integration and physical acceptance obligations.
