# Windows client guide

## Installation

1. Download the current installer from the project's GitHub Releases page.
2. Run the installer on Windows 10 or 11 (x64).
3. Open Palisade from the Start menu.
4. Sign in and verify this device from the Palisade Security Center using a trusted device.

A desktop installation creates its own Matrix device. Keep your recovery information available before removing an older device or clearing the app profile.

## Updating

Choose **Open Installer & Update Manager** from the application or tray menu. After confirmation, Palisade opens its bundled manager outside the installation folder and exits so the manager can update or repair the installation. Cancel leaves Palisade running. From v0.2.4, the client checks signed stable releases ten seconds after launch, every six hours, and after waking if overdue. Failures retry after fifteen minutes with backoff up to six hours. **Check for Updates** in the application or tray menu, or **Check now** in device settings, checks immediately. New releases produce a native notification and visible status; installation remains a confirmed manager action.

The original test executable must be replaced manually with v0.2.2. The new signing key is established by that explicit reinstall; the older key cannot authenticate this bootstrap. See [signing and recovery](UPDATE-SIGNING.md).

## Notifications, microphone and tray

Windows notification settings and your Palisade channel preferences both affect notifications. Experimental voice requires microphone permission in Windows and voice support on the Palisade service. Camera and screen capture are not enabled in this client.

New profiles keep Palisade running in the tray when the window is minimized or closed. Existing saved preferences are respected. Toggle **Minimize and close to tray** in the tray menu to change this behavior. Use **Quit Palisade** or Ctrl+Q to exit completely.

## Security and recovery

Closing or uninstalling Palisade does not revoke a Matrix session. Sign out in Palisade or revoke the device from another trusted session. Local Matrix session and crypto storage remains in the dedicated Electron profile. The existing web session vault does not provide independent TPM-backed protection.

If an update fails verification, the current version stays installed. Do not bypass warnings by obtaining installers from an unrelated website. Check the project's GitHub Releases page or contact the project operator.

If a Matrix decryption or device verification problem occurs, use Palisade's Security Center and recovery process before clearing local app data.

The tray preference survives restarts and applies both to minimizing and closing the window. Enable **Palisade > Launch on login** to start with Windows; it is off by default. **Palisade > About Palisade** shows the installed version. Standard edit shortcuts and Ctrl+R work in the app. Alt+F4 closes to the tray when that preference is enabled; Ctrl+Q exits. The desktop observer converts increases in the hosted app’s unread counts into generic native notifications, subject to Windows settings.

If the manager cannot open, Palisade reports the failure and stays running. Use the official manager to recover. Do not clear the profile to fix an update.

The app fills the window with no title bar or reserved top strip. Compact Tabler settings, minimize, maximize/restore and close icons sit in the upper-right corner. They are grey normally; minimize/maximize turn Palisade blue on hover or keyboard focus, and close turns red. Drag from the invisible top edge to move the window. Press Alt to access the application menu.

Use **Send test notification** in the tray menu to check Windows toast delivery while the app is open. The first move to the tray also shows a native notification; clicking it restores Palisade. Enable message notifications in Palisade and in Windows Settings > System > Notifications. Windows controls the toast position and may suppress banners during Do Not Disturb/Focus Assist. Palisade channel and active-conversation preferences still determine the unread counts available to the observer.

The system tray logo markings change from white to bright red while there are unread messages. Reading them restores white markings. Initial load and the first five seconds of unread synchronization are silent. Later increases generate generic notifications without message text; bursts are limited to one alert per five seconds. The observer reads the existing workspace/direct-message unread labels and exposes no desktop functions to the page. It cannot alert for messages the hosted app immediately marks read or for unread changes that cancel each other out. Windows may cache pinned shortcut artwork; the tray icon updates immediately, while installed shortcut artwork is refreshed by upgrading.

The gear before minimize opens the device settings panel. Change tray behavior or launch on login, send a test notification, open the update manager, or view the installed version there. Close the panel with its close button, the gear, or Escape.

Device settings has three sections: **General** for tray, login and notification testing; **Permissions** for the fixed Windows notification and microphone pages; and **About & updates** for the manager, page reload and a diagnostic summary. The summary excludes account data, messages, profile paths and credentials. Windows privacy settings and Palisade conversation preferences still govern access and delivery.

From v0.2.5, incoming unread increases request native Windows taskbar flashing while the app is unfocused. Windows controls its attention color. Focusing Palisade or clearing unread messages stops the flashing. Notifications register the Palisade display name and an external logo under the existing application ID before sending; upgrade and restart to apply the branding repair.
