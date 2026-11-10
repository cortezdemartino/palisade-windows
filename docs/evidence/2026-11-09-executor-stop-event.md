# Direct executor stop event

Starting Windows revision: `a8bf3fa15602f6310fe5d60fffc6d83eb9817a07`.
That normal-fence revision passed full exact-SHA
[CI42728803272](https://github.com/cortezdemartino/palisade-windows/actions/runs/42728803272).
The preceding custody revision `718348fd7dc4fbb907da7bda897fe09da0500c3a` passed
full exact-SHA [CI42727476589](https://github.com/cortezdemartino/palisade-windows/actions/runs/42727476589).
Companion stays frozen/read-only at `01ae58565dd29dce692bace6d09834c5efdbff69`.
No canonical protocol or fixture was changed.

## Implementation and boundary

Closing the step pipe revokes future writes but does not discard bytes already
buffered in the child. Its 75ms step deadline is not a substitute for native Stop.
ProcessInput now creates a per-generation stop event before spawning and retains
it in the immediate child owner. Fencing latches admission closed, signals the
event, closes stdin even on signal failure and polls the exact child. A signal
error remains an error while the child lives; actual exit is still required before
any owned mirror UP. No kill or timeout-success fallback was added.

Normal `--executor` requires the event argument; the old normal arity is refused.
The diagnostic `--executor-probe` grammar remains unchanged and non-authorizing.
Before joining input ownership, hooks or READY, the child retains a wait-only
observer and exact creator wait handle. Parent duplication corroborates the
DUP_HANDLE process handle with CompareObjectHandles against the already validated
creator and rechecks liveness. The observer is non-inheritable and cannot signal or
reset Stop. Same-user/session/unelevated/fixed-image checks remain unchanged.

The child checks its direct witness before READY, on dequeuing a request and in
ChildInput's validity closure before each privileged primitive. Parent death,
event signal and query errors fail closed without a propagation thread. The
watchdog independently triggers the existing release-only cleanup. Cancellation
does not disable that owned-UP cleanup on an ordinary desktop.

This closes buffered admission after observed Stop. It does not guarantee that an
already-entered SendInput call cannot race the signal, nor establish physical
delivery, host/helper hold arbitration or monitor-owned normal receipts. The
existing normal child still has no dedicated terminal Native receipt. Production
main does not start it. `cooperative_ready` stays false and `localYield` remains
unadvertised.

The event is currently signaled by ProcessInput fence/Drop. RuntimeRevoker and
NativeStop still set atomic flags; the ordinary worker must reach fencing to
signal this particular event. Therefore this qualifies direct fence-to-child
cancellation, not independent hotkey/session-Stop delivery while that worker is
blocked. Before activation, the final monitor/safety plane must register event-only
revocation before READY/DOWN and signal it without gate, pipe or ordinary-worker
waits. That remaining urgent integration is not supplied by this component.

## Executed checks

Formatting/strict Clippy and **175 Rust tests per default/all-feature configuration**
passed (122 library plus 53 integration, including the fixed witness fixture entry
point). Focused tests use a real no-authority Windows child: its stdin stays open
through actual exit, while the creator's event alone changes the child's direct
witness. No AtomicBool, renderer, queued command or EOF relays Stop. Other checks
confirm wait-only/non-inheritable rights, refusal to SetEvent/ResetEvent, stopped
and invalid observer refusal, immediate signal-before-pending-fence and owned-UP
refusal until actual child exit.

**29 process groups per default/all-feature configuration**, separate hidden-startup
visibility, **202 app tests**, local x64 NSIS packaging and packaged native
integrity/release panic checks passed. Full packaged desktop/capture/peer checks
passed on the prior exact-SHA CI and remain required on this new candidate.
Independent source-only review cleared the dormant event/handle boundary and
recorded the remaining urgent-path integration limit above. It ran no builds or
physical tests.

Unsigned development installer SHA-256:
`b384e173a640dc55ae09aefe82e31ddb9bc31e400bad2d23b1d20dc06724e319`.
Packaged helper SHA-256:
`a362507e6b636ae8377b8c2c6f20e6219c55438beb5a748cd6139a974f577ee4`.
Neither artifact was installed or published; these are not replacement v0.3.3 assets.

```powershell
cargo test --locked --manifest-path native/assist/Cargo.toml windows::process_input::tests
cargo test --locked --manifest-path native/assist/Cargo.toml windows::parent::tests::stop_observer
npm run test:assist-native
npm run build:assist
npm run test:assist-process
```

Classification: **Implemented / Verified in simulation**, with real no-authority
Windows event/handle/process checks. No input, hooks, consent or production service
was exercised. Actual native cleanup, full live integration, protected-desktop and
physical cooperative acceptance remain **Not Verified / Planned**. No installation,
release signing, publication, deployment or production activation was performed.
