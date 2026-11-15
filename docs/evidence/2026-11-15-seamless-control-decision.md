# Seamless cooperative control decision

M4G-ASSIST-FINAL-6, Subphase 4. Decision: **Blocked** for the current Windows
SendInput/observation architecture. This is a documented safety limitation,
not proof that every possible Windows design is impossible. No driver, input
filter, privileged service or device was installed or exercised.

The requirement is host-first, immediate safe handoff under unchanged signed
permission, without a fixed idle delay, manual transfer or renewed consent.
The frozen extension v3 localYield contract mandates 2,000 ms of local idle.
Reducing that constant would violate compatibility and would not solve input
ownership. Preserve v3 and its fixtures. A future immediate-handoff mechanism
requires a separately negotiated capability/version and new fixtures, followed
by native and physical qualification before advertising it.

## Delivery boundary

[SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
inserts a sequence of keyboard/mouse events into the shared input stream and
observes integrity-level restrictions. Its return value describes insertion,
not ownership of the resulting logical key/button state. It supplies no
documented conditional operation to release only a particular device's hold.

[GetAsyncKeyState](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getasynckeystate)
queries state at a point in time. A query and a later injection are separate
operations. [Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input)
can identify device sources through queued observations; this is not an atomic
merge or veto over a separately delivered synthetic event.

Both [keyboard](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc)
and [mouse](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)
low-level hooks may be removed silently after a callback timeout. An earlier
canary, heartbeat or physical-state snapshot cannot establish interception of
the next release.

Concrete failing schedule:

1. Palisade observes a neutral local control and tracks a helper-owned hold.
2. A hook stops receiving callbacks or is silently removed.
3. The host's physical DOWN reaches the shared desktop.
4. Helper cleanup injects UP based on its earlier ownership record.
5. A successful insertion count or later receipt cannot prove the host's hold
   remained effective.

Timers, repeated queries and acknowledgements after injection do not eliminate
this schedule. Logical grant ownership and positive process/mirror cleanup remain
necessary, but they are not delivery-time physical ownership proof.

## Virtual HID evaluation

Microsoft's [Virtual HID Framework](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/virtual-hid-framework--vhf-)
supports a kernel-mode KMDF/WDM source driver and virtual HID collections.
The [keyboard/mouse HID stack](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/keyboard-and-mouse-hid-client-drivers)
maps device reports through class drivers. These documents do not specify the
per-device logical hold merge needed to establish this safety property for all
target applications and APIs. A separate virtual device is a research candidate,
not an ownership guarantee.

[VhfReadReportSubmit](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/vhf/nf-vhf-vhfreadreportsubmit)
and its buffer/readiness contract concern report submission and buffer ownership.
They do not attest that helper UP leaves every overlapping physical hold intact.
A privileged HID implementation also cannot assume it inherits SendInput's
integrity restrictions. It would need explicit user/session/desktop enforcement.

A driver candidate adds [kernel signing](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-requirements--windows-vista-and-later-),
reviewed installation/removal, Secure Boot/HVCI compatibility, PnP/power/crash
handling, supported Windows versions and a new privileged attack surface. No
such implementation or installation is authorized by this decision. Do not use
unsigned drivers, suppression of host input or protected-desktop bypasses.

## Required proof before changing the decision

An independently reviewed candidate must demonstrate device-aware safe ownership
merge, host-first cursor collision handling and crash-safe release. Test same-key
and same-button overlap, multiple physical devices, foreign injection, hotplug,
driver/process failure, hook loss, lock/UAC/secure desktop, sleep and terminal Stop.
Every resource/input operation still needs current signed authority, local consent,
lease, source and peer binding. No kernel path may become a universal input writer.

Use approved disposable devices and preserve failed results. Measure actual
handoff latency (median, p95 and worst observed) only after delivery safety is
established. No such measurements or physical acceptance were executed here.

The existing observer, scheduler, fencing and custody remain intact; `localYield`
and production Assist remain unavailable. Windows resource work, companion fixes,
qualification preparation and updater regressions continue independently.
