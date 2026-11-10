# Native cooperative physical observation

Starting Windows revision: `cba547286117cff869cadd905f6c6bde1107d1b7`.
Its full exact-SHA [CI42730524572](https://github.com/cortezdemartino/palisade-windows/actions/runs/42730524572)
passed. Companion inspected read-only at
`c9f822bef6d6257f2a2845b309131e83729a0e42`; docs/54 adds acceptance tooling,
without changing the frozen contract. No production feature switch was changed.

## Contract preservation

Raw git blobs were independently hashed at e45cc8f, frozen 01ae585 and latest web
main before proceeding. All three protocol sources and the yield fixture match:

| Source | Git blob |
| --- | --- |
| protocol.ts | `29bfd8d51d8cf423932379add25b83ae06334d8c` |
| cooperative.ts | `caaadb956475a91c2b7e1776651d7c7a8a88f873` |
| extension.ts | `005ac6ce6ca4b7c4fc27bcc32aa6d9d5f7ad03e1` |

Yield fixture SHA-256:
`5e678a011977aa063bba30200234ae9bb0fd971196daf198143e208155b74e89`.
The original e45 manifest hashes to
`345ce5758b965df67ada0544878c85aab1d922620a7d624cb82405afcdc42ac4`.
Windows retains frozen manifest v3 SHA-256
`f9ac9d835333e341907ae63f2d2a497ef4944607ea60fc344e81ad64115606b0`,
which still matches latest web main. All nine fixture families and all 30 acceptance
cases are preserved. No canonical source/fixture copy was edited.

## Implementation

- `cooperative.rs`: native non-cloneable FenceStamp separates immediate observation
  fencing from deferred numeric decoding. Attaching a LocalEvent never pauses twice.
- `observation.rs`: bounded LL aggregate/per-device raw hold union and ordered
  unresolved journals. Both sources must agree before release; repeat cannot create
  another hold. Stale stamps never lower generation; missing data stays uncertain.
- `windows/physical_activity.rs`: actual Raw Input/message-window and low-level
  hook adapter, with singleton registration ownership, strict packet/device checks,
  native callback stamps, bounded queues and separate lifetime cancellation. No
  input/suppression/consent API or renderer route exists. Native cleanup is fenced
  before release, including startup errors and unwind; failures stay failed.
- `windows/activity.rs`: shared slot prevents overlapping legacy/cooperative
  monitors in one process. Existing execution still uses legacy terminal behavior.

Native baselines are deliberately never qualified by this component. An empty
table, tag, global bitmap or heartbeat is not a proof of per-device neutrality.
Snapshot.require_neutral refuses an unqualified baseline. Independent signed
authority, local consent, lease, peer/source binding and the existing epoch barrier
remain required. Observation data cannot bypass these requirements or authorize
native input. Resources remain independent of ordinary local yielding.

## Verification classification

**Implemented / Experimental**, with simulation of numeric native inputs, parser
packets, queues and worker failures. The new hook/Raw Input registration functions
were not started, and no physical keys, mouse, clipboard, file transfer, consent,
service session or personal desktop input was exercised. Actual hook ordering,
multi-device behavior, baseline and delivery-time enforcement are **Not Verified**.

Focused regressions cover both UP arrival orders, raw-first/LL-first activity,
two keyboards holding Shift, repeat, button holds, partial/unmatched transitions,
device removal/reuse/generation errors, queue overflow/reentry, missing/foreign/
reordered stamps, unsupported scans and existing international mappings. Numeric
packet checks reject truncated/oversized/type-confused data, missing devices,
E1/overrun input and ambiguous coalesced button transitions. Injected matching-tag
or cleanup key-up input is never treated as physical activity/approval. Shutdown
failure stays failed and invalidation precedes cleanup on unwind.

Final local formatting and strict all-target/all-feature Clippy passed, followed by
**193 Rust tests per configuration** (140 library plus 53 integration). The ten
hold-model and eight numeric adapter/queue/lifetime groups are included in that
count. The separate **29 process regression groups per configuration** and hidden
startup check also passed. These real no-authority subprocess regressions exercise
the existing helper, not the new hook/Raw Input registration path. Independent
source-only review cleared the observation slice after identifying/fixing missing
international inverse mappings, stale stamp high-water, timer overtaking, raw-first
pre-query fencing and attempted-registration cleanup. The reviewer ran no builds,
hooks or physical tests.

**202 app tests**, production npm/RustSec audits, local x64 NSIS packaging and
packaged helper integrity/release panic checks passed. The first packaged desktop
run timed out in the picker Cancel click. The fixture awaited executeJavaScript
acknowledgement from a renderer that the picker destroys before resolving its
selection; this is consistent with a close/ack race. The fixture now races that
acknowledgement with the real picker result, then retains the exact ID/null
assertion and original 40s deadline. Earlier script errors still fail. No product
permission or picker behavior changed; independent source review cleared the fix.

After that correction, the full packaged desktop/permission suite passed all
**30 cases**, the isolated capture runner passed **five component runs** (including
**12 negative video/lifecycle cases**) and the owned peer runner passed **14
fake-input cases**. These do not qualify a native Assist session or the new hooks.

Unsigned development installer SHA-256:
`ced3c0e096ed0d2ffc9df655442e904a9876b80ae5f1e20071eb95bd2ce0158b`.
Packaged helper SHA-256:
`d592b7d25ed50083253f8f90048192f041f4bbbe63d9655e495a89636a486fe1`.
The harness-only correction is not packaged product code. Neither artifact was
installed, signed for release, published or used to replace published v0.3.3 assets.

| Acceptance | Current evidence | Remaining gate |
| --- | --- | --- |
| WIN-Y01 | Canonical scheduler simulation plus bounded native observation model | Qualified baseline, delivery arbitration, connected status/runtime and actual host input |
| WIN-Y02 | Both UP orders, two-device/repeat/button hold simulation | Actual helper neutralization/handover with host holds untouched |
| WIN-Y03 | Existing terminal authority fixtures and observer failure/lifetime tests | Native terminal transitions, retained cleanup and real device execution |
| WIN-Y04 | Frozen v3/reference negotiation fixtures retained | Shipped native v3 handler and explicit qualified peer opt-in |

All four remain **Not Verified / Blocked** for real native acceptance; all other
frozen acceptance cases remain preserved. Unit or mock evidence never changes a
device record to Passed.

```powershell
npm run test:assist-native
npm run build:assist
npm run test:assist-process
npm run check
npm run package:win
npm run verify:assist-package
```

## Open delivery enforcement finding

Microsoft documents silent low-level hook removal after timeout and no reliable
removal notification. A hook-thread timer or earlier successful canary does not
prove interception of the next UP. A missing acknowledgement after SendInput is
too late to prove that a host-held key/button was not released. The observer passes
all events onward; it is not the missing delivery arbiter. Do not set
cooperative_ready or advertise localYield based on its output. See
[LowLevelKeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc)
and [registration ownership](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices).

Continue the existing #33/#5 review/device gates with a concrete delivery mechanism
and negative tests for hook loss immediately before overlapping-host cleanup.
Do not test that case on a personal machine or weaken consent/OS boundaries to make
it pass. Initial native baseline qualification, final delivery arbitration, host/UI
status, complete normal monitor/child/mirror custody, independent urgent Stop and
live runtime/resource integration remain unfinished. COOP-1 is not accepted as
complete. Phase U1 remains queued after it. No install, release signing, publication,
deployment or production activation occurred.
