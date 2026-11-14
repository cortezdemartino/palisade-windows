# Palisade Windows 0.3.5

## Clearer Windows settings

- Six categories: General, Notifications & permissions, Updates, What's new,
  About & support, and Account & workspaces.
- Persistent navigation, independently scrolling content and readable controls.
  Smaller windows and enlarged text use a compact layout.
- Labelled switches, keyboard navigation, focus restoration, high-contrast colors
  and reduced motion support.
- Clear Close versus Quit behavior and simpler tray/application menus.
- Separate verified release history from update preferences. Prefer the installed
  release's saved notes and explain offline or unavailable history.
- Keep Update Manager under **Updates > Having trouble updating?** for repair.
- Show the Updates page when checking from the application menu. Confirm reload
  before interrupting calls, retain preferences when saving fails, and confirm
  troubleshooting information was copied.

Existing preferences, application identity and account/session storage are
retained. Hosted account/workspace settings remain in the Palisade web client.
Signed-update verification and device permission boundaries are unchanged.

## Verification and limitations

The UX implementation passed 243 application tests, packaged-source Electron
checks, NSIS packaging and update/native integrity checks on Windows. The
[UX evidence](evidence/2026-11-14-windows-ux-1.md) contains before/after screenshots
and the distinction between actual UI checks and simulated device actions.
The protected release workflow repeats tests, audits, packaging and artifact
verification for this version before signing and publication.

Actual Windows display-scaling changes, Narrator, installed upgrade, UAC
cancellation, post-install relaunch and authenticated session preservation remain
Not Verified on a disposable Windows device. An interrupted installation may
require the bundled Update Manager repair path. Keep account data while repairing;
see [the installed-upgrade procedure](U1-INSTALLED-UPGRADE.md).

Full Assist remains disabled. Updated inactive native components preserve
retained cleanup ownership and stricter process/session checks. This release
does not qualify cooperative input, native resource integration or real-service
cross-client acceptance, and it does not activate those capabilities.

Update Manager remains the pinned v1.0.1 recovery application. Its maintained
source changes are not a new Manager binary in this release.
