# U1 offline native trust and staging foundation

Starting revision: `c92155265e93a7755dfc94881c638ee762ee22cb`. Its signed-notes slice
passed exact-SHA Windows CI [42878783181](https://github.com/cortezdemartino/palisade-windows/actions/runs/42878783181).

## Implemented, inactive library

`native/update` contains independent v3 release/rotation verification using compiled
public trust, transition floor, application identity and package version. A private
verified-release type gates complete SHA-256/SHA-512 installer hashing. Strict
unknown/duplicate-field, version, date, size, filename and location checks remain.
Fresh in-memory keys and actual JavaScript signing output exercise interoperability.

Windows components implement protected user/SYSTEM staging ACLs, immutable stage
paths, handle-relative fixed-name file access, strict updater-directory seals,
separate no-delete installation/ancestor guards, file locks, reparse/hardlink
refusal and bidirectional installation-overlap detection by file identity.

The parent foundation binds creator PID, exact retained process object and creation
time, same unelevated user/session, private pipe owner, image identity/version and
fixed NSIS uninstall registration. Scope uses that uninstall identity, not HKCU
App Paths, which the running application also repairs for machine installations.
PE metadata checks distinguish the x64 client from NSIS's legitimate x86 bootstrap.

There is no stage/apply executable command, desktop dispatcher or installer launch
yet. These components are not packaged or advertised as an operational updater.
The existing Manager remains the routine installation path. Details and limitations:
[native boundary](../NATIVE-UPDATER.md).

## Local verification

Windows x64, Rust 1.98.1, Node 22.23.4. `npm run test:update-native` passed Rust
formatting, strict all-target/all-feature Clippy, and **18 tests in each default and
all-feature configuration**. `cargo audit --file native/update/Cargo.lock --deny
warnings` passed with 34 locked crate dependencies and no reported vulnerability.

Coverage includes:

- Valid and tampered signatures, foreign keys, unknown/duplicate fields, both
  installer hashes, partial/oversized bytes, same-version/downgrade refusal.
- Authorized transitions, required installed prefix, successor authentication,
  activation floors, cycles and actual Node/Rust canonical-byte compatibility.
- Real Windows ACL inheritance/ownership checks and malformed bounded SID parsing.
- Actual NTFS junction rejection, hardlink rejection, retained file write/delete
  refusal, updater directory mutation refusal and installation overlap.
- Real directory tests proving ancestor rename refusal while permitting ordinary
  installed-child overwrite/rename/delete/create. Earlier lock configurations failed
  these tests and were corrected; they were not treated as acceptance passes.
- A real Node-to-Rust no-authority subprocess checking the original creator and
  private pipe owners. Cargo/Node/self cannot authenticate as installed Palisade.
- File-backed registration fixtures for user/machine scopes, HKCU App Paths overlay,
  conflicting records, stale versions, wrong publisher, wrong directory/image.
- Nonexecuted disposable x86/x64 PE fixtures compiled with Windows' C# compiler for
  product/company/version and bootstrap architecture validation. No fixture or
  installer application was executed.

Independent source review found no verifier bypass and required immutable authority,
bounded/inheritable ACL checks and separate directory roles. Those corrections and
negative tests are implemented. A compromised same-user owner or administrator is
outside the guarantee; weaker ancestor guards do not prevent in-place owner changes.
Handle-relative verified reads are not a claim of handle-based process execution.

## Not verified and remaining

Successful authentication of an actual installed parent, full handoff, user/machine
installation, UAC refusal, relaunch, interrupted installation, profile preservation
and a Manager-independent installed update are Not Verified. The fixture scope test
does not write or qualify the actual registry. A pipe test is not installed-parent
acceptance. File hashes and PE metadata alone do not prove a completed installation.

Next implement the bounded stage/apply handshake, one-shot commit/cancel boundary,
retained parent wait, NSIS launch and child ownership, completion/recovery and
relaunch. Connect the desktop state machine/preferences/UI and shared Assist cleanup
barrier, then package and test the actual helper. Actual upgrades require approved
disposable Windows devices. No installer execution, production signing, publication,
deployment, Assist activation or personal-device remote-input testing occurred.
