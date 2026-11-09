# Direct native stop witness

Starting Windows revision: `5d95cdadaad87f4effb99ff0e653b3a903ce93e2`.
Its full [exact-SHA Windows CI](https://github.com/cortezdemartino/palisade-windows/actions/runs/42679846356)
passed. Companion contract remains frozen at `01ae58565dd29dce692bace6d09834c5efdbff69`.

## Implementation

The owned coordinator now retains a wait-only duplicate of its exact validated
monitor process and inherited terminal Stop event. `ProcessStop` checks both
directly; event signal, actual process exit and query error all stop admission.
Handles are non-inheritable, not reopened from a reported PID, and have no event
modification or process termination rights. The token latches detected failure
into the existing launch atomic. The independent watcher remains for eager
cancellation; it is no longer the only observation point for channel commands.

The private channel's reader and ordinary dispatcher use this token. Every
nonterminal command is discarded after Stop. Strict grammar and sequence checks
still run. Terminal Shutdown remains accepted so it can publish its cleanup
sequence/challenge. Independent review caught an initial error here: the monitor
signals the event before forwarding Shutdown, so refusing all frames would have
poisoned normal cleanup. The corrected path also tolerates earlier ordinary frames
still queued in the pipe writer, without executing them.

Normal monitor startup now establishes its one native Stop surface, hotkey and
session notifications before spawning the coordinator. It does not create a second
UI owner. This ordering alone is not an authenticated authority-bootstrap proof.

No identity, session, input, clipboard or file command was added. Private v1 remains
channel-only; v2 remains `nativeInput:false` and `channel-and-process-exit`. A direct
stop witness does not authorize an operation or prove that input was released.

## Verification

Formatting/strict Clippy and **140 Rust tests per default/all-feature configuration**
passed (90 library, 15 executor, 4 input-model, 5 Profile 2, 19 release/ownership,
7 stream authority). New native tests check wait-only event rights, direct event
observation without an atomic propagation tick, sticky Stop, and exact retained
process exit while its event remains unsignaled. One of the three new library tests
is the fixed no-authority child fixture used by the process-exit case.

**202 application tests** and **26 no-input process groups per configuration**
passed, plus hidden-startup visibility. The added process group repeats owned
shutdown eight times with four queued ordinary requests each, exercising native
event-before-Shutdown forwarding. Existing normal/crash/parent-loss/watchdog cases
remain passing. Independent source review cleared the corrected inactive boundary;
it ran no builds or physical tests.

Local x64 NSIS build, packaged native integrity/cleanup probe, desktop/permissions
(30 cases), five capture-component groups with 12 local-video negatives, and 14
owned-DTLS/fake-input peer groups passed.

Unsigned development installer SHA-256:
`77e31624ef9007018a7a01eba031be14077051265962c9762c85c0598266c3fd`.
Packaged helper SHA-256:
`33000fafbffb860791fc8c3c3885ef82f922e436b44de556f3e917182d82ba64`.
These bytes were not installed, signed or published.

## Remaining integration

The future coordinator authority owner needs an authenticated launch-bound monitor
bootstrap after Stop and the positive cleanup registrar are ready. Probe, unowned
and private v1 modes cannot mint that authority. Runtime/executor children must
retain the direct witness at every privileged admission and primitive, not merely
share an atomic checked by a watcher. Each new session must share the launch
witness while allocating a distinct local flag; cloning a session token or creating
an atomic-only child is insufficient for that future registry.

Positive executor-generation/child/mirror receipts, live session/tombstone ownership,
protected consent, resource integration and physical acceptance remain unfinished.
This is verified native process/liveness evidence with **no input authority**,
not real-device Assist acceptance or production qualification.
