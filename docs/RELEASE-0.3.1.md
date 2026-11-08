# Palisade Windows 0.3.1

This desktop update includes the independently reviewed Assist development components. **Full Assist remains unavailable and disabled.** Remote control, native clipboard sharing and native file transfers cannot be enabled by installing this update.

- Add strict native Profile 2 input decoding, expanded physical-key and mouse-button mappings, bounded Unicode handling and tracked cleanup in inactive components.
- Reject stale, replayed and unauthorized input in the native session admission path. Preserve legacy protocol validation.
- Add shared cleanup preparation before Quit and update-manager startup, retaining pending cleanup owners and reporting failures.
- Expand native process, permission, protocol and packaged desktop verification; correct Profile 2 fixture provenance checks.

Messaging, desktop settings, notifications, tray behavior, installation identity, account profiles and signed automatic updates are retained. Update-signing trust is unchanged. Do not clear application data when upgrading.

The Windows x64 installer and signed release manifests are verified by the protected release workflow. Windows Authenticode signing remains optional. Physical installation/upgrade with an authenticated account, real native input, cross-client Assist, working TURN and performance acceptance remain Not Verified. Component tests and simulated input do not establish full Assist readiness. Earlier failed attempts and remaining Windows implementation are recorded in [the component evidence](evidence/2026-11-08-profile2-native.md).

The stopped, unpublished `v0.3.0` tag is preserved. Version 0.3.1 uses a fresh tag and release assets. This release does not activate privileged functionality or deploy changes to the web platform or backend.
