# Native updater boundary

The v0.3.4 development candidate stages updates from the desktop. It uses a
small Windows helper for offline verification and installation after a safe
exit. Update Manager is available for manual repair. See the
[installed-upgrade procedure](U1-INSTALLED-UPGRADE.md) for the remaining qualification.

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
The apply path keeps the file handle protected from writes and deletion until
Windows signals exit of the exact installer process.

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
The dispatcher refuses overlap before stage verification or readiness. At most
three UUID directories may exist. Discard opens files relative to the validated
directory and refuses nested directories, reparse points and hardlinks.

Handle-relative reads bind the verified data object; they do not turn Windows
pathname-based process creation or NSIS destination writes into handle-based
operations. Keep staging anchors protected throughout execution and retain the
explicit hostile-owner exclusion rather than claiming stronger protection.

## Parent and installation binding

The parent authenticator checks the OS-reported creator PID, retains its
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

## Desktop handoff

Electron checks the bundled helper against an integrity record inside the protected
ASAR. It starts the helper with private pipes and a fixed command. The native
helper chooses the cache path. The desktop records the UUID, target and canonical
release digest before downloading, and publishes the installer only after both
hashes and size match. Native inspection checks them again, including the PE
product identity. A downloaded stage can be reverified offline for display.

At a requested restart the desktop re-fetches the current signed release and
matches the exact digest. It waits for the shared Assist cleanup barrier and
profile save before starting the verified staged copy of the helper. A matching
ready message permits a one-shot commit on the private pipe. The desktop exits
after the acknowledgement. The helper waits on the retained parent process
handle, re-verifies the held installer, adds its Internet zone information and
launches it with a fixed NSIS scope and destination. Windows approval and
publisher checks still apply.

The helper and installer share a product-wide exclusion object across Windows
sessions. The installer holds it through exit and consumes a one-use permit
before proceeding. The installer refuses when a Palisade process is running.
The desktop checks for this exclusion before starting a window or admitting
Assist. If the helper integrity or status check fails, built-in installation
and Assist capture are unavailable for that launch.

The helper checks the installed executable version after a successful exit and
reopens Palisade. The next launch confirms the version that actually runs. Local
receipts support diagnosis and carry no installation authority. When a receipt
is lost or a restart is interrupted, the desktop re-verifies saved bytes and
requires a fresh restart authorization. Once commit has been sent, an
unconfirmed acknowledgement is reported as uncertain.

## Recovery and qualification

A failed NSIS run can leave mixed application files. Use the official Update
Manager to inspect and repair the installation without clearing the profile.
If Windows reports launch success without an installer process handle, the
helper keeps the verified files and exclusion until Windows restarts. The
published v0.3.3 installer and uninstaller predate the no-force policy. The
first manual upgrade from that version is a separate compatibility case.

Successful installed-parent authentication, an actual NSIS upgrade, UAC
cancellation, post-install relaunch and authenticated profile preservation
remain Not Verified on an installed test system.
