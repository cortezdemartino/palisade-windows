# Crash and failure diagnostics (Testing Center TC-02)

Status: **implemented, not released.** Verified with simulated Electron objects (node tests) and
with a real Electron 44.7.2 runtime on Linux (headless). **Not verified on Windows** or in a
packaged, signed build. Nothing is sent anywhere by the desktop app itself.

Specification and server side: project-palisade `docs/68-TESTING-CENTER.md` §16–§18,
`docs/69-TC-CLIENT-INTEGRATION-HANDOFFS.md` (Electron), contract
`docs/contracts/tc/diagnostics-v1.json` (copied to `tests/fixtures/tc-diagnostics-v1.json`;
keep the two identical).

## How it works

```
Electron main (src/diagnostics)            hosted Palisade page (apps/web)          Testing Center
 monitor.cjs  detects failures   ──► store.cjs (PC, bounded) ──► bridge.cjs ──► page ──► consent? ──► POST /api/v1/tc/client/crashes
 crashReporter: local minidumps only (never uploaded)
```

1. `monitor.cjs` detects failures with Electron's supported APIs and builds a crash or error
   summary in the Testing Center contract (enumerations and bounded values only).
2. `store.cjs` keeps it on the PC in `%APPDATA%\Palisade\diagnostics\pending` (at most 20
   summaries, 256 KiB, 14 days; atomic writes; invalid or expired files are deleted).
3. The main process never holds the person's Matrix session, so it cannot send anything. The
   signed-in page asks for pending summaries over the `palisade.diagnostics` bridge, sends them
   only if the person allowed crash reports (or diagnostics) for this account, and acknowledges
   them so the desktop app deletes its copies. Without consent they stay on the PC and expire.

A Testing Center outage, a full disk or a diagnostics fault never affects starting, signing in,
messaging, calls, Assist or updates: every hook is wrapped and failures are ignored.

## What is detected

| Failure                         | API / source                                         | Summary `kind` / `process`             |
| ------------------------------- | ---------------------------------------------------- | -------------------------------------- |
| Renderer crash, kill, launch failure | `app` `render-process-gone`                    | `process-crash` / `startup-failure`, `renderer` |
| Renderer out of memory          | `render-process-gone` reason `oom`                   | `oom`, `renderer`                      |
| GPU process failure             | `app` `child-process-gone` type `GPU`               | `process-crash`, `gpu`                 |
| Utility process failure (audio, network, …) | `child-process-gone`                     | `process-crash`, `utility` (subsystem `voice`/`network`) |
| Frozen window (15 s)            | `webContents` `unresponsive` / `responsive`          | `hang`, `renderer`                     |
| Main-process exception or unhandled rejection | `process` `uncaughtExceptionMonitor` (observes only) | `exception` / `unhandled-rejection` (`startup-failure` before the window is ready), `main` |
| Previous run did not end cleanly | session marker (`session.json`)                    | `unclean-exit`, **inferred**           |
| Three unclean ends in 10 minutes | `unclean-starts.json`                               | `crash-loop`, **inferred**             |
| Updater failure                 | updater phase `failed`                               | `update-failure`, `updater`, `UPDATE_FAILED` |
| Native Assist helper failure    | runtime status `Cleanup failed` / `Native endpoint unavailable` | `helper-failure`, `native-helper`, `ASSIST_CLEANUP_FAILED` / `ASSIST_ENDPOINT_UNAVAILABLE` |
| Native crash                    | `crashReporter.start({ uploadToServer: false })`     | local `.dmp` only, never uploaded      |

Not classified as a defect: a clean renderer exit; the renderer ending because Assist revoked
screen capture (`forcefullyCrashRenderer`, marked expected for 5 s); a hang that recovers within
15 s. An unclean shutdown is reported as `inferred`, because Windows updates, power loss and Task
Manager end processes too.

**Unchanged behaviour.** No `uncaughtException` or `unhandledRejection` handler is installed, so
Electron's own handling of an unrecoverable main-process error is exactly as before;
`uncaughtExceptionMonitor` only observes and writes the summary synchronously first. Sandboxing,
context isolation, the trusted-origin checks and the Assist and updater boundaries are unchanged.

## Breadcrumbs

The last 50 events from the last 30 minutes, in memory: main-process events (start, window
ready, renderer or child gone, unresponsive, updater and helper failures) and events the page
forwards (sync, voice join, sign-in outcomes). Each is `{at, id, category, subsystem, action,
outcome, code?}` with fixed identifiers only; anything else is dropped (`schema.cjs`). They are
attached to a summary and stored only when the person later consents to diagnostics on the
server side.

Never recorded: message text, drafts, keys, tokens, passwords, clipboard, remote input, screen
or media content, file names or contents, paths with user names, URLs, hardware serials.

## Bridge (`palisade.diagnostics` v1)

Exposed as `window.palisadeDiagnostics` only to the main frame on `https://palisade.onyxops.org`
(`chat-notifications-preload.cjs`). Members: `info()`, `pending()`, `acknowledge(ids)`,
`breadcrumb(crumb)`, `onRecovery(listener)`. The main process accepts calls only from the trusted
main frame (same checks as the Assist bridge), exact arguments, 10 calls/s and 20 breadcrumbs/s.
The page can read pending summaries and delete them; it cannot read files, minidumps, settings
or anything else.

## Crash recovery

After an unexpected end, the main window shows "Palisade closed unexpectedly" once it is ready.
"Report the problem" asks the page to open **Report a bug** (the request waits until a signed-in
page asks for it). There the person can preview the desktop summaries as part of the diagnostic
attachment and send them with that report only; they are then deleted on the PC. "Not now"
leaves them to expire. In a crash loop the prompt suggests checking for updates or reinstalling.
Security features are never turned off to recover.

## Minidumps

`crashReporter` writes Windows minidumps to `%APPDATA%\Palisade\Crashpad` (Electron's
`crashDumps` path). They are **never uploaded**: `uploadToServer: false`, no submit URL, and the
Testing Center has no dump endpoint (raw dump collection is blocked, project-palisade docs/68 §18).
At start-up dumps older than 14 days are deleted and the total is kept under 64 MiB. Minidumps can
contain message plaintext and keys: do not attach them to GitHub issues or send them to anyone.

## Build identity

Summaries report `client.platform = windows-electron`, `appVersion` from `package.json`,
`buildId = win-<version>` and channel `stable` for installed builds (`internal` in development).
`os.version` is `os.release()` (for example `10.0.26100`). Symbols (`.pdb`) for Electron and the
native helpers can be registered in the Testing Center with `palisade-admin tc-symbols-upload
--platform windows-electron --kind pdb`; they are not processed yet (no minidump processing).

## Verification

| Check | Result |
| --- | --- |
| `node --test tests/diagnostics.test.cjs` (contract vectors, stack sanitizing, store bounds/expiry, unclean/crash-loop, dump sweep, renderer/GPU/utility/hang, expected revocation, exception observation, bridge trust/limits) | 9/9, simulated Electron objects |
| Full `npm run check` | 304/304 |
| Real Electron 44.7.2, Linux, headless (`--ozone-platform=headless --no-sandbox`) | Forced renderer crash → confirmed `process-crash` with breadcrumbs and one local minidump, uploads disabled; expected revocation → nothing stored; `SIGKILL` then restart → inferred `unclean-exit`; four unclean starts → `crash-loop` |
| **Not verified** | Windows runtime, packaged/signed build, real GPU/OOM failures, native helper crashes, the recovery dialog on Windows, end-to-end submission from an installed client |

Controlled crash testing (disposable profile, never production): run Electron with a throwaway
`userData`, call `webContents.forcefullyCrashRenderer()` (renderer crash) or end the process with
Task Manager (unclean exit), restart, and check `%APPDATA%\…\diagnostics\pending`. Do not crash
personal machines or production services without authorization.

## Troubleshooting

- **No recovery prompt after a crash:** the previous run's `session.json` may be missing (first
  run) or marked clean. Diagnostics faults are silent by design.
- **Summaries never leave the PC:** the person has not allowed crash reports for this account, or
  the installation does not accept them (Testing Center collection off or halted).
- **Disk use:** at most 256 KiB of summaries and 64 MiB of minidumps; delete the `diagnostics` and
  `Crashpad` folders in `%APPDATA%\Palisade` to clear them at any time.

## Rollback

Reverting this change removes the bridge and detection; existing `diagnostics` and `Crashpad`
folders can be deleted. Nothing on the server depends on the desktop app sending summaries.

## Telemetry (Testing Center TC-03)

`src/diagnostics/telemetry.cjs` records desktop events in the Testing Center event envelope and,
like crash summaries, only stores them on the PC (`diagnostics/events`, 200 events, 128 KiB,
7 days). The signed-in page reads them over the bridge (`pendingEvents()`,
`acknowledgeEvents(ids)`), adds this account's installation id and sends them only with consent:

| Event | Consent | When |
| --- | --- | --- |
| `app.lifecycle` `launch` (`coldStart`), `ready` (`durationMs`), `quit`, `recover` (`previous`: `unclean`/`crashed`), `migrate` | usage analytics | start, window shown, clean exit, after an unclean end, after a version change |
| `update.lifecycle` `check`/`download`/`install` with `success`/`failure` | diagnostics | updater phase changes (`available`, `staged`, `installing`, `failed`) |
| `performance.measure` `cold-start` | performance | process start to window shown |
| `performance.measure` `memory-mb`, `memory-peak-mb`, `cpu-percent`, `process-count` (all Electron processes, `app.getAppMetrics()`), `queue-size` | performance | every 10 minutes, `idle` or `active` (window focused) |
| `performance.measure` `event-loop-lag` | performance | when the main event loop is late by more than 200 ms |

The hosted page reports its own usage, feature and in-page performance events as platform `web`;
the desktop app does not report "active" use itself, so people are not counted twice. Unclean-exit
summaries now carry the ended session's id as `correlationId`, so session stability is attributed
to the right session.

Not measured on the desktop yet: warm start, IPC latency, renderer responsiveness timings, GPU
utilisation, disk use and native helper resource use (pending; the helper runs outside Electron's
process metrics).

Verification: `tests/diagnostics.test.cjs` (11 tests, simulated Electron objects). Real Electron
44.7.2 on Linux (headless): `app.getAppMetrics()` returns working-set memory and CPU for the
Browser, GPU, Utility and Tab processes as the sampler expects, and one sample takes about 1.3 ms
(every 10 minutes). **Not verified on Windows.**
