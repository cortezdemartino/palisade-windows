# Native updater boundary

Status: an inactive library foundation under `native/update`, not an installed
updater. There is no apply command or connection from the desktop yet. Existing
routine updates still use Manager. This document records implemented boundaries
and the remaining handoff requirements, without claiming installation acceptance.

## Implemented trust

The Rust verifier compiles in the existing public root, installed transition prefix,
application identity, NSIS uninstall GUID and package version. No network call,
cache or command argument can replace them. It requires strict schema 3, stable
numeric versions, a strictly newer version, the exact versioned Windows x64 asset
name and GitHub location, bounded size, canonical time and both installer hashes.
Duplicate and unknown JSON fields are rejected. Delegations require the original
authorized prefix, predecessor signatures, increasing activation floors and no
cycles. Native signing bytes are tested against the actual JavaScript signer.

Only verification can construct `VerifiedRelease`; its payload is immutable to
callers. Installer hashing accepts that verified type, not an arbitrary payload.
The eventual apply path must keep the returned file handle protected from writes
and deletion until installer execution finishes.

## Private staging and directory roles

The new staging namespace is the Windows-known LocalAppData folder followed by
`Palisade-Updates-v1` and a validated random UUID v4. It is separate from the roaming
Electron account/session profile and the Assist identity store. Existing user data
is not moved. Both updater-owned levels require a protected, inheritable DACL
containing only the current user and SYSTEM with full access and the intended
owner. Existing broad, malformed or non-inheriting ACLs are rejected.

Updater-owned directories use strict read sharing, with directory-list access to
make Windows sharing checks effective. They prevent in-place reparse mutation,
rename and child rename while retained. Publish staged content before apply locks;
create receipts under fixed exclusive names or retain their exact write handles.
Receipts are descriptive, never installation authority.

Ancestor and installation directory handles allow read/write sharing but deny
delete sharing. Windows tests prove they block ancestor rename while permitting
ordinary child overwrite, rename, delete and directory replacement. These handles
do **not** prove that an owner cannot change an ancestor in place. A compromised
same-user owner or administrator remains outside the security guarantee. Never
use this weaker mode for an updater-owned staging level.

Each stage file is opened by one fixed component relative to the retained stage
handle, through the [documented user-mode NtCreateFile API](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile).
This is a normal user-mode system call, not a driver. The helper rejects reparse
points and hardlinks, and retains deny-write/delete sharing on verified files.
Stage paths are immutable in the API. Installation/staging overlap is detected in
both directions by retained directory file identities, including pathname aliases.
The eventual command dispatcher must refuse any overlap before readiness.

Handle-relative reads bind the verified data object; they do not turn Windows
pathname-based process creation or NSIS destination writes into handle-based
operations. Keep staging anchors protected throughout execution and retain the
explicit hostile-owner exclusion rather than claiming stronger protection.

## Parent and installation binding

The inactive parent authenticator checks the OS-reported creator PID, retains its
exact process handle, rejects PID reuse through creation times, requires the same
unelevated user and nonzero interactive Windows session, checks private pipe owner
PIDs, and validates the Palisade image's product/company and compiled version.
The retained image lock is released only after the exact parent process exits.

App Paths corroborates the image by file identity. It does not decide installation
scope, since desktop Start-menu repair also writes HKCU App Paths for a machine
installation. Scope comes from the fixed builder-derived NSIS uninstall GUID,
matching publisher, display name/version and directory identity. Conflicting,
stale, missing or wrong-location records fail closed. Normal user/machine and
HKCU-overlay combinations are tested with file-backed registry-record fixtures;
those tests never modify the actual installation registry.

Product metadata is an additional identity check, not a signature. Client images
must be Windows x64 PE files. NSIS installers can have an x86 bootstrap containing
the authenticated x64 application package; their exact bytes and x64 asset identity
remain covered by the signed release and release-build checks. Do not mistake
bootstrap machine type for the packaged application's architecture.

## Unfinished handoff and qualification

Implement the fixed stage/apply commands, bounded private handshake, one-shot
commit, exact-parent exit wait, concurrent-apply exclusion, scope-preserving NSIS
execution, installed-version confirmation, relaunch and failure receipts. Do not
pass `--force-run`, accept arbitrary executable paths or use the old PowerShell
helper. Preserve Windows security dialogs and UAC.

The desktop coordinator must revalidate current release metadata, preserve the
shared Assist cleanup/profile barrier, and never launch the apply helper before
that barrier succeeds. Cancellation must stop before apply is committed; after
that point report installation in progress rather than imply it was canceled.

Actual successful installed-parent authentication, NSIS upgrades, UAC cancellation,
crash recovery, relaunch and profile preservation are Not Verified. No installer
has run, and no production signing, deployment or publication is authorized in U1.
