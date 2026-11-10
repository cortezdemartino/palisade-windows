# Retained normal input fencing

Starting Windows revision: `718348fd7dc4fbb907da7bda897fe09da0500c3a`.
Companion remains frozen/read-only at `01ae58565dd29dce692bace6d09834c5efdbff69`.
No wire contract, canonical fixture or production feature switch changed.

## Defect and correction

The dormant normal ProcessInput used `Child.kill()` during startup failure and
fencing. This could destroy the child's cleanup-only release owner. The new
ExecutorProcess owns the exact child immediately after spawn, before extracting
pipes, starting a reader or interpreting readiness. Every subsequent error closes
ordinary input and retains the created process through its actual exit.

Admission fencing and positive reaping are separate states. Backend::fence first
latches admission closed and closes stdin, then polls the original child handle.
While it remains alive, the result is `executor-cleanup-pending`; it does not wait
under SessionGate or allow mirror UP. Query/wait errors retain ownership. Existing
runtime cleanup retries outside the gate. Drop waits without a kill or timeout-
success fallback. The child's existing EOF/heartbeat watchdog continues its
release-only cleanup, waiting for Default desktop if necessary.

Only actual reaping enables the existing owned-only mirrored release path.
Ordinary DOWN and unowned UP remain refused after fencing. This does not turn exit
into a monitor Native receipt or qualify the old child's protected-desktop/physical
cleanup. Normal monitor-owned spawning, a single dedicated terminal receipt owner
and retained coordinator mirror acknowledgements remain required.

The same edit corrects an unused probe constructor's argument count: the fixed
`--executor-probe` takes no normal-owner argument. Normal input still requires its
owner, build feature and later registered Executor authority.

## Reproduction and evidence boundary

Four Windows unit groups use fixed hidden BCL-only subprocesses and private
temporary marker files. No input, hooks, enrollment, clipboard or resource API is
called. The children write a marker only after observing EOF and finishing their
simulated cleanup; killing them would fail the assertion. Cases cover:

- Nonblocking fence with a child held alive until explicit test permission, no
  heartbeat or mirror UP before actual exit, then owned-UP-only cleanup.
- Malformed, wrong-sequence and denied readiness.
- Actual three-second readiness timeout and missing output pipe.
- Dropping an active owner retains EOF cleanup and exact child exit.

The fixture is prepared before measuring native startup, leaving its three-second
deadline unchanged. The exit-gated case cannot pass merely because a delay elapsed.
It does not test physical stuck keys or protected-desktop transitions.

Local formatting/strict Clippy and **172 Rust tests per default/all-feature
configuration** passed (119 library plus 53 integration). **29 process groups per
configuration**, hidden-startup visibility, **202 app tests**, local x64 NSIS
packaging and packaged native integrity/release panic checks passed. The temporary
marker fixture subsequently gained exclusive directory creation to refuse stale
evidence; the four focused tests and strict Clippy were rerun after that test-only
change. Independent source review cleared the inactive fence/ownership correction;
the reviewer executed no builds or tests. Full packaged desktop/capture/peer checks
passed locally at the preceding custody checkpoint and remain required in the new
exact-SHA Windows CI.

Unsigned development installer SHA-256:
`8474ef4352fed655375f84b9730850caadfbc3b22b3726fb18fb05cc86a12cdd`.
Packaged helper SHA-256:
`b75ddb31e3024112c50b8cb53392b1065307e36af057d27a43aede3c297a81b1`.
These describe the local package built before the final test-only directory change;
no production source changed afterward. Neither artifact was installed or published.

```powershell
cargo test --locked --manifest-path native/assist/Cargo.toml windows::process_input::tests
npm run test:assist-native
npm run build:assist
npm run test:assist-process
```

Classification: **Implemented / Verified in simulation** for the backend policy,
with actual no-authority Windows process waits. Physical input cleanup and complete
monitor/runtime integration remain **Not Verified / Planned**. No candidate was
installed, release-signed, published, deployed or enabled in production.
