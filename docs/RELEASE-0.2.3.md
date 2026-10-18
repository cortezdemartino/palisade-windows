# Palisade Desktop v0.2.3

- Add compact Tabler window controls and a device settings panel with General, Permissions, and About & updates sections.
- Add tray and login controls, notification testing, Windows notification/microphone settings, and a diagnostic summary without account data.
- Fix packaged controls loading through a restricted local protocol. Preserve the existing installation identity, browser sessions and application data.
- Include the independently verified Palisade Installer & Update Manager 1.0.1 for manual updates and repairs. The client delegates release checks and installation to the manager; it does not schedule its own background checks.
- Verify the manager dependency by a pinned SHA-256 and size. Require unit, visual interaction, archive and signed artifact verification in the Windows release workflow.

The manager verifies signed release metadata and installer integrity before installation. Windows Authenticode signing is optional and is not applied to this release. Existing old-key test clients still need the previously documented manual bootstrap; the pinned public key is unchanged from v0.2.2.

Automated tests and Windows packaging are verified. Interactive installation, authenticated-session migration and native Windows banner delivery require separate operator testing; they are not represented as passed by the automated checks.
