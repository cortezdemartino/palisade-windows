# Local executor registration and cleanup authorization

Starting Windows revision: `5046a0d0d95ddae6da2f31bae580c4542a84d863`.
Its exact-SHA Windows CI [42620269751](https://github.com/cortezdemartino/palisade-windows/actions/runs/42620269751)
passed all steps, including the strengthened parent-loss test and packaged checks.
The historical [failed run and investigation](2026-11-09-native-parent-loss.md) remain recorded.

This slice connects the existing native gate and executor through a local execution
identity and terminal cleanup authorization. It is not a crash-surviving monitor
receipt, a completed fresh-grant workflow or live Assist activation.

## Implementation

`SessionGate::reserve_executor` creates a unique, linear reservation before the
dormant runtime starts its input child. The reservation is consumed by the exact
registered Executor. A still-unconsumed reservation can be canceled only if the
gate has not admitted a registered permit. Cancellation invalidates earlier permits.
Another reservation cannot replace an existing or cleanup-pending executor.

Registered execution permits bind the gate instance and exact executor identity in
addition to the original input epoch, signed grant, source, action and deadline.
The real ProcessInput adapter requires registration; its no-authority probe keeps
the legacy test path. A permit minted before registration cannot be used by the
new executor. Startup rechecks its original input epoch, current authority and both
terminal flags after child readiness before publishing native readiness.

After all old input grants are revoked, the gate can issue one pending cleanup
requirement with a unique nonce and exact executor identity. It is not cloneable,
deserializable or a page-supplied flag. Only that registered executor can return its
one-shot receipt after terminal backend fencing/reaping, owned input release and
the final mirror acknowledgement. A fresh empty executor, another gate, ordinary
yield or failed mirror synchronization cannot satisfy the requirement.

Acceptance removes only the retired executor registration and invalidates its old
permits. It does not restore a grant, extend the original deadline, replace the
controller, clear terminal atomics or consent to a new peer. Resource permissions
and their independent revocation are preserved. A requirement remains meaningful
when the resource epoch changes or the underlying session ends; cleanup never
restores ended authority.

Release retries retain the same owner and requirement. They keep checking protected
desktop/session loss and set global stop independently of the gate lock while
waiting for release-only cleanup. Final cleanup clones the source before acquiring
the gate, avoiding an inverse lock order with native consent. Impossible registration,
replay or disabled-adapter failures remain unproven instead of retrying as success.

## Verification and classification

Seven new native fake-backend groups passed in the first full checkpoint, bringing
the release/ownership suite to 19. They exercise:

- One-shot requirements/receipts, mirror failure and resource/deadline preservation.
- Empty and foreign executor refusal.
- Wrong-gate receipts without clearing either registration.
- Linear cancellation, admitted-permit cancellation refusal and resource-only refusal.
- Permits minted before registration.
- Nonterminal yield distinguished from terminal cleanup authority.
- Input cleanup across independent resource revocation or session end.

The final runtime guard corrections passed the complete candidate verification:
formatting, strict Clippy and **127 Rust tests per configuration** (77 library,
15 executor, 4 input-model, 5 Profile 2, 19 release/ownership and 7 stream-authority).
**202 application tests** passed. The native build and **25 no-input process groups
per configuration**, including exact retained-handle parent-loss checks and the
hidden-startup fixture, passed.

NSIS packaging, packaged native integrity/cleanup probe, desktop/permission checks
(30 cases), all five isolated capture groups and 14 actual-DTLS/fake-input peer cases
passed. These remain inactive/component tests; no installer was executed.

Local installer SHA-256:
`34e46537b461170d8015f6c8430911d2873290035a01a8b2ebb3a11ebfa92ef5`.
Packaged helper SHA-256:
`653b4158f1545b320eef6185a0b58713a740aac4ed3156137b8694667e61d0c4`.
These are unpublished development artifacts with maintenance version 0.3.3.
The new exact pushed-SHA CI result is recorded separately in the issue tracker. Independent source review
cleared the dormant registration boundary after restoring desktop-loss handling and
checking startup/lock ordering. No reviewer hardware test or native build is claimed.

Physical Windows input, UAC/lock transitions, native startup/consent and real-service
reacquisition remain **Not Verified**. Fake backend acknowledgements do not establish
operating-system delivery or physical overlap safety.

## Remaining accepted work

The local tokens cannot be used as a wire cleanup proof. The direct monitor still
negotiates only `channel-and-process-exit` with native input false. Before live
activation it must retain exact executor processes and authenticated generation-bound
positive receipts across coordinator/child failure. Process exit alone is insufficient.

Fresh-grant admission is still blocked. The next integration must preserve these
constraints from the frozen companion contract:

1. A legitimate replacement observed while cleanup is pending must be held as a
   non-authorizing offer inside the same gate. It must not make LeasePoller terminate
   unrelated resources; forged or malformed records still fail closed.
2. The service keeps at most eight current capability records and overwrites a
   replaced capability. Retain bounded immutable revoked records for stale-response
   validation without allowing old IDs/nonces to re-enter authority.
3. One new approval can issue mouse and keyboard at the same newer generation.
   Preserve the original approvedAt/deadline and monotonic acquisition floor while
   admitting that cohort. Partial input approval/replacement must be handled explicitly.
4. Require fresh native consent and a proven new input connection. The existing
   resource-peer verification must remain independent; a repeated fingerprint or
   old verify_peer call cannot unlock a new input acquisition.
5. Use new per-acquisition stop and LocalYield owners after accepted cleanup.
   Never reset an old terminal flag, revive old consent or silently resume input.

Continue the real native runtime, physical arbitration, capture/consent and all eight
resource operations. No install, production activation, signing or release is authorized
by this checkpoint.
