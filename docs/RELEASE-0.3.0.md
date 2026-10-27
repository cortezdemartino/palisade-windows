# Palisade Windows 0.3.0

Unpublished. The release attempt was stopped at the owner's request. The existing v0.3.0 tag is not a published release and must not be moved or reused for a different candidate.

- Add restricted Assist capability reporting compatible with the Palisade web application.
- Harden permission handling so a capture feature flag cannot grant screen access or bypass source selection.
- Add inactive source-picker and session/data validation components for continued Assist development.
- Add inactive isolated capture with app-owned video/certificate/socket checks and source shutdown fixtures, plus native consent/input/resource components with separate permission handling and crash-surviving controller exclusion.
- Expand automated checks for permissions, window controls, notifications and signed updates.

Assist screen/window sharing, remote mouse/keyboard control, clipboard sharing and file transfer are disabled or unavailable in this release. Capability reporting does not authorize these actions. Native execution and activation still require their separate security review and acceptance tests.

Existing messaging, desktop settings, tray behavior, notifications and signed updates are retained. Application identity, account/profile paths, persistent Electron session and update-signing trust are unchanged. Do not clear application data when upgrading.

The release pipeline tests and packages Windows x64, signs the update manifests, and verifies installer hashes before publication. Windows Authenticode signing remains optional. Installation and upgrade with a real authenticated account, live cross-client Assist and physical remote-input acceptance have not been verified for this release.
