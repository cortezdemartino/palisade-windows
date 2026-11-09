# Native parent-loss launch policy and exact-process verification

Starting revision: `ab94c6fda59d37e94c47ae18cbd21d4f519c6372`.
That candidate's Windows CI [42617329627](https://github.com/cortezdemartino/palisade-windows/actions/runs/42617329627)
failed its native process step: 24 of 25 process groups passed; the parent-loss test
reported PID 504 still present after four seconds. Native formatting/Clippy and
120 tests per configuration had passed. Later CI stages did not run.

## Investigation

The first local reproduction passed. The old assertion used PID existence rather
than retained process identity, so it could not distinguish an actual survivor from
PID reuse. Independent source review then identified a real launch-policy defect:
the Assist launchers did not specify `detached: true`.

Node 22.23.4's [bundled libuv Windows launcher](https://raw.githubusercontent.com/nodejs/node/v22.23.4/deps/uv/src/win/process.c)
assigns non-detached children to an additional kill-on-parent-exit job. This can kill
the direct monitor before its native parent-loss cleanup finishes. Detached launch
skips that additional libuv assignment; it does not request
`CREATE_BREAKAWAY_FROM_JOB` or bypass externally imposed Windows job restrictions.

An independent controlled diagnostic forced detached false versus true with an
in-memory spawn shim, the same default helper bytes and original process handles
retained before terminating only a disposable Node fixture parent:

| Forced policy | Monitor | Coordinator | Stop-event-only holder |
| --- | --- | --- | --- |
| Non-detached | exit 0 at 27 ms | exit 101 at 54 ms | exit 0 at 30 ms |
| Detached | exit 101 at 58 ms | exit 125 at 43 ms | exit 0 at 5 ms |

These observations establish different monitor termination behavior and expose a
broken-stderr panic. They did not reproduce a holder surviving four seconds; the
precise cause of the original CI timeout is not conclusively established by them.
An earlier uncontrolled diagnostic overlapped a source edit and is not treated as
a controlled baseline.

## Corrections

- Both Assist launchers use detached launch while keeping hidden windows, private
  stdio pipes and referenced lifecycle ownership. They do not call child `unref`.
- The helper's fixed error diagnostic uses best-effort `writeln` with an ignored
  I/O result. Losing the parent diagnostic pipe no longer replaces deliberate
  exit 125 with a Rust logging panic and exit 101.
- The parent-loss fixture waits until a read-only observer retains the original
  monitor, coordinator and holder process handles and their creation times. Only
  then may its disposable Node parent exit or be forcibly terminated.
- Both clean exit and abrupt fixture-parent death must lead to the exact retained
  processes exiting within the original four-second deadline. The required codes
  are monitor 125, coordinator 125 and event-only holder 0. A job-killed monitor
  cannot silently satisfy the assertion merely by disappearing.
- The observer never signals or kills observed processes. Its .NET process handles are
  standard retained handles; no reduced-access handle claim is made. It validates the fixed
  test image and waits the retained handles, not reopened PIDs. PowerShell's execution
  policy and Windows security settings are unchanged.

Two initial failures while improving the harness were corrected without changing
product checks: a newline in the child-script template was escaped incorrectly,
and Windows PowerShell's array pipeline behavior initially nested the input list.
The corrected focused test then detected monitor exit 101, reproduced the logging
defect, and passed after the best-effort diagnostic fix.

## Verification

The rebuilt default helper passed the focused exact-handle parent-loss case with
the intended exit codes. The complete native formatting/strict Clippy and **120 tests per configuration**,
**202 application tests**, and **25 no-input process groups per default/all-feature
configuration** passed. The parent-loss group now includes both normal exit and
forced termination of only its disposable Node fixture parent. The revised
stdout-drain check separately passed the focused parent-loss group again.
Independent review cleared the source boundary and passed 10 Node VM checks.
NSIS packaging, packaged native integrity/cleanup probe, the full desktop/permission
runner (30 cases), all five capture-component groups and 14 owned-DTLS/fake-input
peer groups passed. The exact-handle clean/forced parent-loss case also passed against
the actual packaged helper and the manifest extracted from its ASAR. No installer
was executed.

Local development installer SHA-256:
`b4ee29bc9d20623bdb820103090280d53109e4646be750a96e2e65efd6718901`.
Packaged helper SHA-256:
`02d717b2c78e44995d77d412d130ae3874db277a9c4fc350fbeb1556d623593a`.
These are unpublished development artifacts, not replacements for published v0.3.3.
The successor exact pushed-SHA CI is recorded separately in the issue tracker. These tests use no-authority fixtures only;
no OS input, clipboard data, production service, private signing key or installer
execution is involved.

This correction does not establish positive input cleanup receipts or complete the
live Assist runtime. The [cooperative checkpoint](2026-11-08-cooperative-core.md) and
the [continuation plan](../ASSIST-RELEASE-PLAN.md) still apply. Production activation,
physical acceptance, signing and publication remain separately gated.
