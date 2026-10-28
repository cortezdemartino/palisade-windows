# Private native Assist channel

Status: **Implemented, inactive, channel-only**, October 28, 2026. This is the first W2 component in [the full delivery plan](ASSIST-RELEASE-PLAN.md). It does not complete W2, start a native session or enable remote control. The production application does not import it.

## Boundary and launch

`src/assist/live-channel.cjs` is a trusted-main module. It verifies the existing ASAR-protected helper size/hash manifest and registers an application lifecycle holder before verification or process creation. A concurrent quit/update aborts creation. The native helper reuses creator PID/creation-time, image, SID, interactive-session, elevation and inherited-pipe checks. No inbound listener or executable command API is added.

Diagnostic guardian v1 remains separate, including its 16,384-byte frame bound and diagnostic kill/exit semantics. Those semantics are never a fallback for the new channel. The new modes are `--live <parent PID>` and `--live-probe <parent PID>`. Normal mode requires the expected packaged Palisade parent. Probe mode permits the existing test-parent exception and cannot open an identity, enroll, display consent, launch an input executor or access clipboard/files, even when sensitive Cargo features are compiled.

## Closed protocol

The private wire uses a four-byte little-endian length prefix, at most 65,536 bytes, strict UTF-8 JSON, exact typed fields and monotonically increasing u32 request sequences starting at zero. This is not the hosted page protocol and supplies no page-facing operation. There is no general request/sign/read/write API in the JavaScript return value.

| Command | Result and purpose |
| --- | --- |
| `hello` | Exact `palisade.assist.live`, version 1, fresh 32-byte challenge and matching mode; reports `nativeInput: false`, `cleanupScope: channel-only` |
| `pulse` | Process liveness only; never session authorization |
| `status` | `authority: none`; this revision has no session operations |
| `shutdown` | Revokes synchronously in the reader; after channel cleanup, returns the launch challenge and `scope: channel-only`, then exits successfully |
| `probe-wait` | Probe-only bounded cancellable scheduling fixture; no native side effects |

Unknown/duplicate/extra fields, invalid challenges, replay, exhaustion, wrong types and incompatible versions fail closed. Empty commands use explicit empty struct variants because Serde internally tagged unit variants otherwise accept extra fields. The main-side closed reply vocabulary additionally refuses duplicate/noncompact JSON, wrong correlation, scope/nonce substitution, malformed UTF-8, excessive frames and output after the terminal proof.

## Cancellation and pressure

The native reader never blocks on the ordinary queue. Its capacity is eight; overflow revokes and fails. Pulses and shutdown do not enter that queue. Shutdown first sets atomic revocation, then publishes immutable completion metadata, then tells dispatch to close. Dispatch cannot race ahead of that publication. Probe waits observe revocation and cancel; they cannot delay shutdown by their requested duration.

A separate writer has ten bounded output slots. Saturation revokes instead of retaining unbounded replies. The independent watchdog checks real parent liveness, clock rollback and a five-second fresh pulse deadline even while stdin is incomplete, ordinary dispatch is waiting or stdout is blocked. No diagnostic immediate-exit watchdog is reused. At this channel-only stage, faults exit without successful cleanup proof; native authority is absent.

The main-side API bounds ordinary outstanding calls to eight, reserves shutdown outside that budget and keeps one pulse in flight. Request expiry/cancellation invalidates callers before waiting. Broken or blocked input closes that pipe so native EOF/watchdog can revoke; stdout continues to drain. The broker never kills the helper, destroys its output to manufacture completion or substitutes elapsed time for exit.

## Completion and failure

The lifecycle holder resolves only after a matching terminal cleanup reply **and** actual successful process closure with no invalid/trailing output. A proven process-creation failure may also remove the no-child holder. A crash, nonzero/signal exit, wrong proof or protocol fault retains failed ownership and postpones quit/update. There is no force-complete or security-bypass option.

`channel-only` is an intentionally limited proof. This helper starts no sensitive child, so it cannot prove that held keys/buttons were released, file I/O finished or descendants exited. Neither a clean guardian exit nor this proof may be accepted for active native sessions. The current broker does not negotiate a broader scope.

## Required before attaching the runtime

- Retain authenticated ownership of every executor/cleanup descendant before authority can start. Guardian crash must not hide a surviving child. A non-killing, no-breakaway Windows Job Object is a candidate; verify actual accounting after closing admission rather than relying solely on notifications/job signaling. A handle-publication design must close the crash-before-child-report race. This remains implementation work.
- Split runtime atomic revocation from retained cleanup/join. Prompt drops, gate locks, file sweeps, network waits and thread joins cannot run on the urgent reader/watchdog path. Existing runtime worker/poller join failures must be treated as failed cleanup, never ignored success.
- Keep SessionGate/LeasePoller as one grant registry. Complete source/enrollment/generation binding, guarded local UI, purpose-bound owned-answer signing, observed-certificate checks and input readiness before dispatching a packet.
- Extend private commands/results only with those enforced prerequisites and explicit negotiated cleanup ownership. Do not widen `channel-only` by changing a feature boolean. Complete real disposable-device acceptance and the existing owner activation gate.

## Verification

`npm run check` includes main-side channel/lifecycle tests. `npm run test:assist-native` checks Rust framing/ingress plus the existing native suite. `npm run test:assist-process` repeats both diagnostic and live process suites against the default release helper and an all-features debug helper. Tests cover repeated adjacent handshake/shutdown, eight stalled requests, independent pulses, malformed/partial/replayed frames, saturated queues/output, real parent death and an unproven probe crash. Only known non-authorizing fixtures may be reaped by a failed test harness.

These are real Windows process/pipe tests and fake-main unit tests, not real-input, native consent, cross-client encryption, installed-upgrade or performance acceptance. See [test evidence](TEST-RESULTS.md).
