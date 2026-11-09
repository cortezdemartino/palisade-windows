# Native cooperative input

Status: scheduler and executor integration **Implemented / Verified in simulation**.
Physical observation, native delivery arbitration, connected runtime and native UI are
**Planned / Not Verified**. The packaged endpoint does not advertise `localYield`.
Existing local-activity termination and production feature switches remain unchanged.

## Canonical contract

The protocol sources originate at `e45cc8f20b3060e8231ecafc29c2417ee6f42040` and are unchanged in the read-only frozen revision `01ae58565dd29dce692bace6d09834c5efdbff69`.
Windows pins the exact control protocol, cooperative reference, extension validator,
yield fixture and interoperability manifest. All earlier v1/v2/Profile 2/release
fixtures remain unchanged. The manifest preserves the earlier 23 acceptance cases
and adds WIN-Y01 through WIN-Y04. The frozen manifest v3 has 30 total cases, also including native peer loss (WIN-L06), endpoint crash (WIN-L07) and active-session updates (WIN-U01).

Extension v3 and endpoint enrollment must declare `localYield` only after the actual
native path qualifies. Every peer must additionally opt in with `input-barrier`
epoch zero before its first ordinary input. A legacy peer keeps terminal local-input
behavior. The control protocol remains v2; the feature is separate from Profile 2.

## Scheduling and authority

```mermaid
stateDiagram-v2
    [*] --> Negotiating
    Negotiating --> Active: verified barrier 0
    Negotiating --> Legacy: first ordinary input
    Legacy --> Ended: local activity
    Active --> Yielded: native local activity, fence execution
    Yielded --> Yielded: local held, recent activity, uncertain cleanup
    Yielded --> Ready: 2000 ms idle, neutral, cleanup complete, authority unchanged
    Ready --> Active: verified barrier for this epoch received after readiness
    Ready --> Yielded: new local activity, next epoch
    Active --> Ended: terminal interruption
    Yielded --> Ended: terminal interruption or authority change
    Ready --> Ended: terminal interruption or authority change
```

`cooperative.rs` owns scheduling and bounded local-hold state. It does not own a
second helper input ledger, grant registry or executor. `SessionGate` remains the
signed authority. Its opaque semantic snapshot binds the gate instance, session,
input authority epoch, capture geometry/generation and input grants, generations,
local consent and lease coverage. Routine renewal of the same valid covering lease
does not change that snapshot. Independent resource bookkeeping is excluded.

Barriers pass current grant, device/session/peer, consent, generation, sequence,
lease and sender-age checks in `SessionGate`. They never become injectable Actions.
Their sender timestamp uses release-like age handling. A delayed post-readiness
barrier receives a fresh short dispatch window; its original native ingress time
still prevents a queued pre-readiness barrier from becoming valid later.

The scheduler checks semantic authority on every admission, including releases and
barriers, in addition to ticks. An authority change cannot be hidden between a
readiness event and the next timer. Epoch exhaustion, stale observation provenance
and terminal interruptions cannot reactivate the old scheduler.

An independent atomic `InputFence` invalidates expanded shortcut/text primitives
without revoking service grants. Native observation stamps its event before ordinary
queue processing. Processing one event cannot acknowledge a later unseen local hold.
Lost observation continuity stays uncertain; foreign or replayed observation handles
end admission. Native callbacks still need to be connected to this boundary.

## Ownership and retained cleanup

`Executor::dispatch_cooperative` reuses existing permits, primitive expansion and
grant-scoped ledgers. It requires an explicitly qualified backend. The existing
Windows and subprocess adapters return false, so this path cannot accidentally
enable cooperative injection. Native-fence interruption is a handled nonterminal
drop; arbitrary backend or clock errors keep terminal failure behavior.

`yield_owned` first requires nonterminal backend quiescence. Each owned release must
receive a positive native disposition: released, or handed over to the host without
an injected UP. Only then are the existing local and per-grant ledgers updated.
Uncertain delivery and failed mirror synchronization keep cleanup pending. Terminal
cleanup and Drop retain the owner and retry the final mirror even after the local
ledger becomes empty. No helper DOWN is automatically reissued after a barrier.

These adapter obligations are not satisfied by a bitmap check or `SendInput`'s
insertion count. The delivery arbiter must protect ordinary input, executor cleanup
and the guardian's surviving mirror. A host DOWN racing a helper UP must never be
released by stale snapshot-based cleanup.

## Remaining native integration

1. Finish monitor-owned executor generation registration and positive terminal
   cleanup receipts before admitting active input. Job/process exit alone is not proof.
2. Implement independently owned physical observation and bounded delivery arbitration.
   Track scan/extended identities, buttons and uncertain provenance before the first
   remote DOWN. Preserve observation across ordinary yields. Never use synthetic
   cleanup events or renderer `isTrusted` as local consent.
3. Qualify known-neutral startup, actual versus injected events, foreign injection,
   low-level hook timeout/removal, multiple physical keyboards/mice, AltGr/IME and
   protected-desktop transitions. `GetAsyncKeyState` alone cannot establish ownership.
4. Connect the persistent native runtime, native Stop/consent, extension v3 display
   events and the peer's monotonic endpoint status sequence. Bind readiness to actual
   quiescence/mirror acknowledgements and verified current authority.
5. Run WIN-Y01 through WIN-Y04 on an approved disposable device. Until then keep
   `localYield` unadvertised and real-device acceptance Not Verified.

Clipboard, files and viewing remain independently authorized. Ordinary yield must
not call `suspend_input`, the terminal backend fence, or a service revocation path.
Stop, revocation, lock/protected desktop, sensitive native UI, lease/peer/source loss,
session replacement, expiry and stream end remain terminal and require fresh authority.

See [evidence](evidence/2026-11-08-cooperative-core.md),
[runtime plan](ASSIST-RELEASE-PLAN.md) and [device qualification](https://github.com/cortezdemartino/palisade-windows/issues/5).
