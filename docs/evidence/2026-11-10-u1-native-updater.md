# U1 native updater candidate verification

The Windows baseline was `e534314cef83ee4d849980c02f833005fbd93f71`.
The signed changelog and Manager source integration were verified at
`c92155265e93a7755dfc94881c638ee762ee22cb`. Offline trust and staging were
verified at `87dd2910be3ca59a82fd523abb9c0fa14ded086e`. The linked Windows
workflows completed successfully.

- [Changelog workflow](https://github.com/cortezdemartino/palisade-windows/actions/runs/42878783181)
- [Native verification foundation workflow](https://github.com/cortezdemartino/palisade-windows/actions/runs/42888442126)

The integrated v0.3.4 candidate passed 222 application tests and syntax
checks with Node 22.23.4 on Windows x64. The native updater passed strict
Clippy and 26 tests in each configuration. The locked Rust update audit
and production npm audit reported no advisories. Manager source passed 51
tests and isolated packaged changelog checks in the earlier slice.

The installed-scope and update coordinator fixtures model valid signed
metadata and separate verification from restart authorization. On Windows,
native tests exercised private directory/file permissions, ownership and
retention of handles, exact pipe creator checks and one-shot exclusion.
The no-install NSIS probe refused a running disposable process without
closing it, accepted one fresh permit and refused replay. It also refused
unknown and malformed permits. Windows x64 NSIS packaging succeeded.
Packaged verification checked the ASAR sources, helper integrity and the
actual packaged helper's version and foreign-parent refusal. The packaged
Electron Settings fixture exercised progress, restart and cancellation
controls against a controlled state. The hosted page had no access to them.

Source review identified the need to exclude cache operations during an
install, retain a lost-acknowledgement handoff until an authenticated idle
report, and await the cookie store flush before exit. The implementation
addresses those cases and tests the restart/repair refusal and profile flush
ordering. Installed upgrade remains a separate qualification.

The test suite contains no positive installed upgrade, cross-user UAC
approval, authenticated profile migration or post-install relaunch evidence.
See [installed-upgrade qualification](../U1-INSTALLED-UPGRADE.md).
The Manager source update is available through the reproducible integration
kit. The published Manager and its bundled pin are the existing v1.0.1
artifact.
