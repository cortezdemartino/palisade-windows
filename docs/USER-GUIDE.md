# Windows client guide

## Installation

1. Download the current installer from the project's GitHub Releases page.
2. Run the installer on Windows 10 or 11 (x64).
3. Open Palisade from the Start menu.
4. Sign in and verify this device from the Palisade Security Center using a trusted device.

A desktop installation creates its own Matrix device. Keep your recovery information available before removing an older device or clearing the app profile.

## Updating

Palisade checks for newer versions at startup and every six hours. Choose **Palisade > Check for updates** or use **Check for updates** from the tray icon to check manually.

A verified update is downloaded and offered for installation. Choose **Install update** to run the installer or **Later** to continue working. Save ongoing work before installing. The installer can close and replace the existing application.

The installed 0.1.0 release cannot update itself. Install version 0.2.0 manually once to enable this feature.

## Notifications, microphone and tray

Windows notification settings and your Palisade channel preferences both affect notifications. Experimental voice requires microphone permission in Windows and voice support on the Palisade service. Camera and screen capture are not enabled in this client.

Closing the window normally exits Palisade. To keep it running in the tray, enable **Keep running when window is closed** in the tray menu. Use **Quit Palisade** to close a hidden instance.

## Security and recovery

Closing or uninstalling Palisade does not revoke a Matrix session. Sign out in Palisade or revoke the device from another trusted session. Local Matrix session and crypto storage remains in the dedicated Electron profile. The existing web session vault does not provide independent TPM-backed protection.

If an update fails verification, the current version stays installed. Do not bypass warnings by obtaining installers from an unrelated website. Check the project's GitHub Releases page or contact the project operator.

If a Matrix decryption or device verification problem occurs, use Palisade's Security Center and recovery process before clearing local app data.
