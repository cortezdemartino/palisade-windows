# Device settings

## Scope and layout

The gear before the window buttons opens a local panel. General contains the saved tray preference, Windows login registration and notification test. Permissions opens the fixed Windows notification and microphone settings pages and explains the client's permission boundaries. About & updates provides the installed version, signed update manager, page reload and a copyable diagnostic summary.

Settings apply immediately. Login remains disabled by default. Existing tray choices, installation identity and the persistent Palisade browser partition are preserved. Notification delivery remains subject to Windows and conversation preferences. The panel never claims to override operating system permissions.

## Security and recovery

The hosted website has no privileged bridge. Only the isolated local controls view can issue the fixed actions. The local protocol serves three explicitly named interface files from the application archive. Navigation, subframe commands, arbitrary URLs and arbitrary file requests are denied. Diagnostics contain app/runtime versions, architecture, tray/login choices and update status, excluding account identifiers, paths, message contents and credentials. No profile clearing or destructive reset action is offered.

## Verification

Unit tests cover sender validation, settings actions, fixed Windows destinations, diagnostic fields and manager dependency substitution. The Windows smoke test loads the packaged archive with release file-protocol restrictions, checks each icon's visible pixels, switches panel sections, exercises a tray setting, dispatches diagnostics and verifies panel closing and window actions. The release workflow requires these checks before signing and publishing.

Version 0.2.4 adds a manual update check alongside the manager, live polling status, and notification delivery status. Automatic checks never install an update. A separate isolated preload observes unread badges without exposing desktop controls to the hosted page. It sends counts only, which drive the red unread markings and generic native notifications.
