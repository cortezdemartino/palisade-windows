# Windows client guide

## Installation

1. Download the current installer from the project's GitHub Releases page.
2. Run the installer on Windows 10 or 11 (x64).
3. Open Palisade from the Start menu.
4. Sign in and verify this device from the Palisade Security Center using a trusted device.

A desktop installation creates its own Matrix device. Keep your recovery information available before removing an older device or clearing the app profile.

## Updating

Palisade checks for newer versions at startup and every six hours. Choose **Palisade > Check for updates** or use **Check for updates** from the tray icon to check manually.

Download progress appears in the tray and application menus and on the taskbar. A native Windows notification announces a verified update, and the installation prompt includes release notes. Choose **Install update** to run the installer or **Later** to continue working. Save ongoing work before installing. The installer can close and replace the existing application.

The original test executable must be replaced manually with v0.2.2. The new signing key is established by that explicit reinstall; the older key cannot authenticate this bootstrap. See [signing and recovery](UPDATE-SIGNING.md).

## Notifications, microphone and tray

Windows notification settings and your Palisade channel preferences both affect notifications. Experimental voice requires microphone permission in Windows and voice support on the Palisade service. Camera and screen capture are not enabled in this client.

Closing the window normally exits Palisade. To keep it running in the tray, enable **Minimize and close to tray** in the tray menu. Use **Quit Palisade** to close a hidden instance.

## Security and recovery

Closing or uninstalling Palisade does not revoke a Matrix session. Sign out in Palisade or revoke the device from another trusted session. Local Matrix session and crypto storage remains in the dedicated Electron profile. The existing web session vault does not provide independent TPM-backed protection.

If an update fails verification, the current version stays installed. Do not bypass warnings by obtaining installers from an unrelated website. Check the project's GitHub Releases page or contact the project operator.

If a Matrix decryption or device verification problem occurs, use Palisade's Security Center and recovery process before clearing local app data.

The tray preference survives restarts and applies both to minimizing and closing the window. Enable **Palisade > Launch on login** to start with Windows; it is off by default. **Palisade > About Palisade** shows the installed version. Standard edit shortcuts, Ctrl+R and Alt+F4 are available in the native menu. Website notifications use the native Electron notification integration and remain subject to Windows settings.

After choosing **Later**, check for updates again to reopen the install prompt. Restarting Palisade removes its disposable download cache and downloads a fresh verified installer if needed. Failed checks appear in update status; manual failures also show a dialog. The application waits for shutdown before running the installer and asks NSIS to relaunch it after success. If Windows blocks installation, launch Palisade again or run the official installer manually. Do not clear the profile to fix an update.
