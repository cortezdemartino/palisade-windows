# Palisade Windows 0.3.4

- Download and prepare signed stable updates in the background.
- Show download progress, the last successful check and installation history
  in **Updates & What's New**.
- Pause automatic updates, cancel a download, retry a failed update and choose
  when to restart. Installing and reopening when quitting is optional.
- Show authenticated release summaries, expandable details and cached history
  in desktop Settings.
- Verify the signed manifest and both installer checksums again in a dedicated
  Windows helper before installation. Wait for Assist cleanup, profile saving
  and the exact application process to exit.
- Refuse installation while another Palisade client is running. Retain
  uncertain update ownership and provide the manual Update Manager repair path.

Installed-upgrade testing, UAC cancellation, post-install relaunch and
authenticated session preservation still require verification on a disposable
Windows test device. See [the qualification procedure](U1-INSTALLED-UPGRADE.md).
An interrupted NSIS installation can require manual repair.

Full Assist remains disabled. Native safety components in this release include
independent executor Stop events and stricter cleanup evidence. Cooperative
input, the normal executor receipt channel and the complete live runtime are
still under development.

Update Manager remains available as the pinned v1.0.1 recovery application.
The shared changelog is implemented in its maintained source and will appear
in a separately published Manager update.

The Windows verification workflow passed on the implementation commit
`a45228dd47d3e931833093cf243458e91528438b`. The protected release workflow
repeats tests, audits, packaging and artifact verification before publication.
