# Device settings

## Scope and layout

Open **Settings** using the gear beside the window buttons, the tray menu or the
Palisade application menu. The app-owned panel keeps navigation visible while
its content scrolls. It remembers the category and scroll position while the
application is running. Close it with its close button, the gear or Escape.

| Category | Contents |
| --- | --- |
| General | Open when Windows starts; keep running in the tray; Close versus Quit explanation |
| Notifications & permissions | Notification test; Windows notification and microphone settings; conversation-preference explanation |
| Keybinds | Record, clear, enable and reset the device-permissions shortcut; optional background registration; unavailable call-action explanation |
| Updates | Installed/latest versions; check, download, cancel and restart; pause; automatic preferences; installation history; optional Manager repair |
| What's new | Authenticated release summaries, dates, details and saved release history, including offline/unavailable states |
| About & support | Version/build/runtime; truthful Assist status; reload; copy troubleshooting information; fixed guide/support links |
| Account & workspaces | Where to find the hosted app's account and workspace settings; Return to Palisade |

Desktop preferences apply immediately on this PC. Startup remains off by default.
With the tray option on, Minimize and Close hide the window and keep notifications
available. Use **Quit Palisade** to exit completely. With the option off, Minimize
keeps the taskbar window and Close prepares the app to exit. Closing to the tray
never triggers the install-on-quit preference.

The panel uses the companion's graphite surfaces, blue accent, 4-pixel spacing and
familiar controls. Wide layouts use side tabs; narrower layouts use rows of tabs.
On very short, enlarged-text layouts, a labelled category selector preserves room
for content. The native window minimum is 640 by 440 device-independent pixels.
The Settings close button and navigation remain available while content scrolls.
Arrow keys and Home/End move between tabs; Tab/Shift+Tab stay in Settings while it
is open. Reopening restores focus to the chosen category. Normal closing returns
focus to the gear; **Return to Palisade** returns keyboard focus to the hosted app.

## Updates, notes and recovery

Updates uses the built-in signed updater as the normal workflow. Downloading an
update does not install or restart it. Restart asks for confirmation and waits for
safe assistance cleanup and profile saving. The optional ready-update-on-quit
preference reopens Palisade after installation; it is postponed while a screen
share is active. Pause lasts 24 hours. Preference changes preserve the current
updater operation's existing safety rules.

**Having trouble updating?** contains **Open Update Manager**, for installed-app
repair. Manager is no longer duplicated in the tray and top-level application
menu. It still confirms exit and uses the same cleanup/integrity barriers.

What's new reuses authenticated release-note history. It initially selects the
installed release when saved, retains an explicit selection while state refreshes,
and renders summaries/details as text. Missing current-version notes, empty saved
history and offline states are explained. No unsigned fallback was added.

## Security and support

The hosted website has no desktop-settings bridge. Only the isolated app-owned
controls view can send the fixed actions. Support destinations are fixed GitHub
URLs; notification and microphone destinations are the existing fixed Windows
URIs. No page-controlled URL, file path or general navigation command is accepted.
Account/workspace settings remain hosted; no unsupported route was invented.

Copy troubleshooting information includes app/runtime versions, architecture,
tray/startup choices and update status. It excludes accounts, message contents,
credentials and profile paths. Reload asks before interrupting calls or sharing.
Save/apply failures report an error and retain the previous saved preference.
No settings file migration, reset or profile clearing is required.

Windows and conversation preferences still govern notification delivery. The test
reports Windows acceptance rather than promising a visible banner. This panel
cannot grant OS access or change account-level notification settings.

## Verification

`npm run test:window-controls` includes the actual Electron Settings fixture,
against source or the packaged archive. It exercises every category/action,
progress/disabled/offline states, text-only notes, focus and real Chromium key
input, minimum-size layout, text enlargement, emulated high DPI/forced colors,
reduced motion and the accessibility tree. Screenshots are captured and inspected;
unit tests also cover persistence, atomic-write failure and sender isolation.
Physical DPI switching and a manual Narrator session are separate from emulation.
See [the UX evidence](evidence/2026-11-14-windows-ux-1.md).

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
