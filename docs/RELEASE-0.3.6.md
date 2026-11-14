# Palisade Windows 0.3.6

Fix the installation-recognition check used by the built-in updater. The official
installer writes a Windows display name such as **Palisade 0.3.5**. The helper
previously required **Palisade**, so it rejected the installation before creating
the download directory. Update checks could succeed while downloads failed.

Both exact official forms are now recognized: **Palisade** and **Palisade followed
by the exact installed version**. Publisher, installed version, installation
directory, executable identity and unambiguous installation scope are still
verified. Signed manifests and installer size/SHA-256/SHA-512 checks are unchanged.

## Getting the fix

Clients running v0.3.4 or v0.3.5 need one final update through **Open Update
Manager** or the official v0.3.6 installer. Their existing helper cannot prepare
the download containing its own fix. Close Palisade normally before running the
installer. Keep the existing installation scope/path and account profile.

In v0.3.6, ordinary checks, downloads and restart controls remain inside Palisade.
Update Manager remains an explicit repair option. No registry edits, replacement
keys or disabled verification are needed.

## Verification and limitations

The mismatch was observed in the real installed v0.3.4 registration. A real
startup check discovered v0.3.5 but could not create a download stage; no Manager
or installer was launched. The native regression failed with that exact name
format before the fix and passed afterward. It also rejects wrong name versions,
suffixes, extra spaces, unrelated products and ambiguous installation records.

The release workflow repeats native/app tests, audits, packaging and signed
artifact verification before publication. This evidence does not establish a
completed installed upgrade. UAC/relaunch/session-preservation acceptance, actual
Windows scaling and Narrator remain Not Verified. Assist remains disabled.
