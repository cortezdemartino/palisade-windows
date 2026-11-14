# Windows client guide

## Updates and release notes

Open **Settings > Updates**. **Check now** checks the official stable release.
In **What's new**, when authenticated editorial notes exist, select a
version under Release history and expand Release details. Publication dates,
categories and compatibility information come from that signed document.
Previously verified notes remain available offline. Missing or invalid notes are
shown as unavailable; an unsigned release description is never substituted.

The candidate shows the last successful check, download progress and installation
history under Updates. It still needs an installed Windows upgrade test.

## Installation

1. Download the current installer from the project's GitHub Releases page.
2. Run the installer on Windows 10 or 11 (x64).
3. Open Palisade from the Start menu.
4. Sign in and verify this device from the Palisade Security Center using a trusted device.

A desktop installation creates its own Matrix device. Keep your recovery information available before removing an older device or clearing the app profile.

From v0.3.3, the installer refreshes Palisade's Start menu entry on upgrades, and the packaged app repairs its own shortcut registration while reusing a correct all-users entry. Search for **Palisade** in Start, then use **Pin to Start** or **Pin to taskbar** in the Windows context menu. You can also right-click the running app's taskbar button and pin it there. Palisade leaves pin placement to you. Windows may take a short time to refresh Search after registration changes.

An old development entry named Electron can incorrectly share Palisade's identity. The app removes that entry only when its app ID belongs to Palisade and its target is the development Electron executable. Other applications' shortcuts are preserved. Development runs now use a separate identity. If registration cannot be repaired, run the official installer again; do not clear your account profile or Windows Search database.

## Updating

Versions 0.3.4 and 0.3.5 contain an installation-name mismatch that prevents
built-in download preparation. Use Update Manager or the official v0.3.6
installer once to obtain the repair. Keep the same installation path and profile;
no account-data reset is needed. From v0.3.6, use the built-in update controls
for ordinary updates and reserve Manager for repair.

Version 0.3.4 checks signed stable releases on launch, every six hours
and after waking if overdue. It checks again after network failures with a
longer delay between attempts. Select **Check now** to check immediately.
Background downloads are enabled by default. Use **Pause for 24 hours** to
postpone checks and downloads, or **Cancel** to stop the current download.

When a download is ready, choose **Restart to update** after saving your work
and finishing any calls. Palisade verifies the release again and waits for
assistance cleanup and profile saving. **Install a ready update when I quit** is
optional and applies only to a ready update when assistance is inactive.
Published clients through v0.3.3 still use Update Manager.

Use **Open Update Manager** under **Settings > Updates > Having trouble updating?** to
inspect or repair an installation. Palisade confirms before closing and
opens the bundled manager after its exit preparation succeeds.

If exit preparation is still pending, Palisade stays open. **Quit pending: stopping assistance** in the menu/tray means cleanup is still running; Palisade exits automatically when it completes. A failure shows a keep-open message so you can retry Quit or the update action afterward. The update manager is not opened while preparation is incomplete. Exit preparation never automatically resumes assistance permissions. Full Assist remains unavailable in this development checkpoint as described below.

The original test executable must be replaced manually with v0.2.2. The new signing key is established by that explicit reinstall; the older key cannot authenticate this bootstrap. See [signing and recovery](UPDATE-SIGNING.md).

## Notifications, microphone and tray

Windows notification settings and your Palisade channel preferences both affect notifications. Experimental voice requires microphone permission in Windows and voice support on the Palisade service. Camera and screen capture are not enabled in this client.

New profiles keep Palisade running in the tray when the window is minimized or closed. Existing saved preferences are respected. Change **Keep Palisade in the system tray** under **Settings > General** to change this behavior. Use **Quit Palisade** or Ctrl+Q to exit completely.

## Security and recovery

Closing or uninstalling Palisade does not revoke a Matrix session. Sign out in Palisade or revoke the device from another trusted session. Local Matrix session and crypto storage remains in the dedicated Electron profile. The existing web session vault does not provide independent TPM-backed protection.

If an update fails verification, the current version stays installed. Do not bypass warnings by obtaining installers from an unrelated website. Check the project's GitHub Releases page or contact the project operator.

If a Matrix decryption or device verification problem occurs, use Palisade's Security Center and recovery process before clearing local app data.

The tray preference survives restarts and applies both to minimizing and closing the window. Enable **Open Palisade when Windows starts** in **Settings > General** to start with Windows; it is off by default. **About & support** shows the installed version and build information. Standard edit shortcuts and Ctrl+R work in the app; reload asks for confirmation. Alt+F4 closes to the tray when that preference is enabled; Ctrl+Q exits. The desktop observer converts increases in the hosted app’s unread counts into generic native notifications, subject to Windows settings.

If the manager cannot open, Palisade reports the failure and stays running. Use the official manager to recover. Do not clear the profile to fix an update.

A compact 36-pixel strip keeps the window controls clear of the hosted app. There is no title text. Compact Tabler settings, minimize, maximize/restore and close icons sit in the upper-right corner. They are grey normally; minimize/maximize turn Palisade blue on hover or keyboard focus, and close turns red. Drag from the invisible top edge to move the window. Press Alt to access the application menu.

Use **Send test notification** in Settings > Notifications & permissions to check Windows toast delivery while the app is open. Minimizing or closing to the tray is silent; the tray icon remains available to reopen Palisade. Message notifications still work, and clicking one restores the app. Enable message notifications in Palisade and in Windows Settings > System > Notifications. Windows controls the toast position and may suppress banners during Do Not Disturb/Focus Assist. Palisade channel and active-conversation preferences still determine the unread counts available to the observer.

The system tray logo markings change from white to bright red while there are unread messages. Reading them restores white markings. Initial load and the first five seconds of unread synchronization are silent. Later increases generate generic notifications without message text; bursts are limited to one alert per five seconds. The observer reads the existing workspace/direct-message unread labels and exposes no desktop functions to the page. It cannot alert for messages the hosted app immediately marks read or for unread changes that cancel each other out. Windows may cache pinned shortcut artwork; the tray icon updates immediately, while installed shortcut artwork is refreshed by upgrading.

The gear before minimize opens **Settings**. General contains tray and startup
preferences; Notifications & permissions contains notification testing and Windows
privacy links. Updates contains built-in checks/downloads/restart, automatic
preferences and installation history. What's new contains verified release notes.
About & support contains versions, reload, troubleshooting information and support
links. Account & workspaces explains where to find settings in the hosted app.

Use the sidebar or category selector to switch pages. Escape closes Settings;
reopening keeps your category. Arrow keys and Home/End move between category tabs.
Preferences apply immediately on this PC. Startup is off by default. Closing to
the tray keeps Palisade running; **Quit Palisade** exits. Update Manager remains
available under Updates > Having trouble updating? for repair, with its existing
exit confirmation and safety checks. Reload also asks before interrupting calls.

From v0.2.5, incoming unread increases request native Windows taskbar flashing while the app is unfocused. Windows controls its attention color. Focusing Palisade or clearing unread messages stops the flashing. Notifications register the Palisade display name and an external logo under the existing application ID before sending; upgrade and restart to apply the branding repair.

## Assist in the Windows client

The separate disposable test build has an experimental screen/window picker.
It stops sharing when its source or native helper ends. This is qualification
work, not a feature enabled in the installed maintenance release. Native control,
clipboard and file workflows still require further integration and acceptance.

Native Assist screen/window sharing is not supported in this release. Use an approved browser for enabled browser Assist; Windows viewing has not been qualified. A desktop capture denial is expected and must not be bypassed. The client exposes no remote keyboard/mouse capability. See [the integration status and browser workflow](ASSIST-INTEGRATION.md).

## Full Assist availability

The planned interaction lets the host keep using their mouse and keyboard while a helper assists, with local input taking priority. Ordinary local activity should temporarily yield remote input; explicit Stop ends permission. This cooperative behavior is not available in the current release.

Planned direction: an eligible stream viewer can request to assist, or the streamer can ask the group or a selected person for help. The streamer chooses the helper, permissions and duration up to two hours. Ending that stream ends every associated permission. This workflow is not enabled in the current Windows build.

The device now reports Assist v1 capabilities, with capture and native endpoint unavailable. This build cannot control another desktop or transfer clipboard/files. No guest virtual keyboard, control approval, clipboard sync or file save workflow is active. Those actions require separately approved native/backend capabilities; do not interpret accepting a call as permission. The inactive screen-sharing stop action is not a certified native-input emergency stop. Continue using the approved browser view-only workflow where enabled, and end assistance to leave it. See the integration guide for blocked/unverified states and owner review steps.

The build includes inactive native consent/input/resource components and an isolated capture provider for upcoming Assist integration. It does not run during normal use and does not enable control, clipboard or file transfers. No extra setup or key copying is required.

## Assist development candidate

Full native Assist remains unavailable in this build. The November 8 component implements extended keyboard, Unicode and mouse-button handling, but it is not connected to the hosted application. There is no setting that safely enables it. Do not install a privileged test build on a personal desktop. Once a disposable-device candidate is approved, follow its recorded setup and acceptance procedure; an ordinary update must not silently enable native control.
