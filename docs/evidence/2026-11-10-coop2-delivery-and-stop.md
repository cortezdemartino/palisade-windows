# COOP-2 delivery decision and Stop ownership

The COOP-2 input baseline is `e534314cef83ee4d849980c02f833005fbd93f71`.
The work continued after U1 at `f194e0caaf5308e9206ff8e90187ca94a358c150`.
Companion main was inspected read-only at
`c9f822bef6d6257f2a2845b309131e83729a0e42`. The frozen wire protocol and
the nine fixture families and 30 acceptance cases are retained.
The Windows manifest bytes match companion main, with SHA-256
`f9ac9d835333e341907ae63f2d2a497ef4944607ea60fc344e81ad64115606b0`.

## Physical delivery remains blocked

Consider a helper-held Shift or mouse button. The host appears neutral at
the last observation. The host then presses that same control before the
helper's synthetic UP reaches the shared input stream. The application
ledger cannot prevent that UP from affecting Windows' logical state.
Windows documents that `SendInput` inserts a batch in order and reports
how many entries were inserted. The API has no conditional delivery or
per-owner release. A snapshot from `GetAsyncKeyState` describes a point
in time and cannot reserve the state for a following insertion.
[SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
and [GetAsyncKeyState](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getasynckeystate).

Low-level hooks can observe impending delivery. Windows can silently remove
either hook after a timeout. A successful callback or heartbeat says nothing
about whether the next event will pass through the hook. Raw Input identifies
physical devices and supports held-state reconciliation, but its asynchronous
delivery does not gate the shared input stream. Reporting uncertainty after
injection cannot undo an UP. Using `BlockInput` would suppress the host's
physical controls and falls outside the design.
[Keyboard hook](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc),
[mouse hook](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc),
[Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input),
and [BlockInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-blockinput).

No verified nonprivileged same-desktop arbitration mechanism was found.
Observe-only components and simulated dispositions cannot satisfy the
delivery requirement. Cooperative input remains unavailable, including
`localYield` at enrollment and the related native status advertisement.
WIN-Y01 through WIN-Y03 remain blocked on safe delivery. WIN-Y04 also needs
real endpoint negotiation and qualification.

## Stop and custody

The existing local Windows safety window registers the exact executor Stop
event before the normal dormant worker can start. Stop and Windows security
transitions signal registered events without taking the session, worker,
pipe or consent locks. The executor retains that same event. Input-only
suspension signals only its own event and leaves resource and lease flags
independent. Lease loss also signals the event. A failed Windows signal
remains recorded through cleanup.

This fences subsequent input admission. A system call already executing when
Stop arrives cannot be recalled. The no-authority tests exercise the real
Windows event and hidden safety-window message path without keyboard or
mouse injection. Previously tested child-process witnesses and saturated
pipe probes remain separate evidence.

An armed native custody entry now requires the exact child exit, its
terminal release receipt and verified coordinator mirror settlement. A
missing witness keeps that entry pending even after process death. The
same registry refuses a replacement native generation or another session
while the earlier executor is unresolved. Probe mode retains its separate
no-authority rules.

A mirror synchronization failure on a registered native backend blocks
readiness and terminal receipts even after a later mirror write succeeds.
The test verifies that retrying the mirror never sends a second release.

## Remaining qualification

`npm run test:assist-native` passed formatting, strict all-target
all-feature Clippy, and 195 tests in each configuration on Windows x64.
The new tests use real Windows events and simulated ledger dispositions.
They establish Stop signalling and proof refusal, with physical delivery
and cross-client qualification tracked separately.

Normal executor spawning and dedicated terminal receipts still need to be
connected to the monitor's retained process handles and to the exact
write-ahead ledger. Do not issue a native receipt from an ordinary channel
ACK, a local cleanup result, a simulated disposition, exit code or job
population. A mirror failure must remain pending until the final design can
prove settlement of that exact generation. Complete persistent session,
consent, peer, source and resource integration before advertising readiness.

Physical overlap, native status interoperability and the docs/54 real
service harness remain Not Verified. They require an approved disposable
Windows device, test issuer and test service. The production issuer, TURN
and integrated security review remain separate gates.
