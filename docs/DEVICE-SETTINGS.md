# Device settings

## Scope and layout

Desktop controls occupy a separate 36-pixel strip above the hosted page, with no title text. The strip provides window dragging and disappears in fullscreen. The gear before the window buttons opens a local panel. General contains the saved tray preference, Windows login registration and notification test. Permissions opens the fixed Windows notification and microphone settings pages and explains the client's permission boundaries. Updates & What's New provides the installed version, update progress, installation history, signed release notes and manual recovery through Update Manager.

Settings apply immediately. Login remains disabled by default. Existing tray choices, installation identity and the persistent Palisade browser partition are preserved. Notification delivery remains subject to Windows and conversation preferences. The panel never claims to override operating system permissions.

## Security and recovery

The hosted website has no privileged device-settings bridge. Only the isolated local controls view can issue the fixed actions. The local protocol serves a fixed allowlist of interface files from the application archive. Navigation, subframe commands, arbitrary URLs and arbitrary file requests are denied. Diagnostics contain app/runtime versions, architecture, tray/login choices and update status, excluding account identifiers, paths, message contents and credentials. No profile clearing or destructive reset action is offered.

The candidate checks and downloads in the background by default. The user can change those preferences, pause for 24 hours, or cancel the current download. Restart is confirmed by default. Installing when quitting is optional. Both a native update and the manual Manager action wait for application exit preparation. Failed preparation leaves Palisade open and postpones the handoff. A pending quit appears in the application or tray menu.

## Verification

Unit tests cover sender validation, settings actions, fixed Windows destinations, diagnostic fields and manager dependency substitution. The Windows smoke test loads the packaged archive with release file-protocol restrictions, checks each icon's visible pixels, switches panel sections, exercises a tray setting, dispatches diagnostics and verifies panel closing and window actions. The release workflow requires these checks before signing and publishing.

Version 0.2.4 adds a manual update check alongside the manager, live polling status, and notification delivery status. Automatic checks never install an update. A separate isolated preload observes unread badges without exposing desktop controls to the hosted page. It sends counts only, which drive the red unread markings and generic native notifications.

## Assist availability

The separate disposable qualification build now connects native screen/window
selection to the isolated capture provider. It has no production activation
toggle. Capture requires an app-owned picker; a window share grants no desktop
input scope. Native input, resources and cooperative input remain unavailable
in the maintenance package. Real cross-client acceptance is pending.
[Current qualification scope](evidence/2026-11-13-rc3-source-session.md).

This candidate has no native Assist capture setting or source chooser. Capture remains denied pending the Electron permission-path review and lifecycle qualification. The v1 description API reports unavailable. Do not interpret the existing microphone setting or notification test as screen-sharing consent. See [ASSIST-INTEGRATION.md](ASSIST-INTEGRATION.md).

The packaged guardian foundation has no activation toggle and is not started by normal application use. Its inclusion does not change the unavailable Assist report or grant any device permissions. Full Assist controls remain unavailable until the canonical authority and native acceptance gates are complete.

Assist remains unavailable in the current Windows build. There is no setting that authorizes native control or bypasses the hosted capture policy. Internal component tests do not enable remote input, clipboard monitoring or file access. Once integrated, each session must display its independently approved permissions and expiry; accepting a call or viewing a stream does not grant those permissions.

Minimizing or closing to the tray stays silent. The unread icon and message notifications remain active according to the existing settings. Send test notification remains available for checking banners explicitly.
