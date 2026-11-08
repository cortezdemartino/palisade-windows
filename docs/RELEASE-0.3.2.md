# Palisade Windows 0.3.2

- Register the installed application consistently for Windows Start, Search, taskbar grouping and user-selected pinning, with explicit executable, icon and relaunch details.
- Repair the Start menu entry during upgrades and packaged startup. Remove an old development `Electron.lnk` only when it incorrectly uses Palisade's application ID; preserve unrelated Electron shortcuts.
- Separate development Electron identity from the installed application so test runs cannot take over Palisade's Windows app entry.
- Add standard App Paths registration, installer location metadata and Onyx Operations Group publisher metadata. Remove owned App Paths entries on uninstall without deleting a successor installation's entry.
- Keep minimizing and closing to the tray silent. Message notifications and the explicit notification test remain available.

Windows decides where pinned apps appear. Palisade does not pin itself automatically or change Windows pinning policies. Search may take a short time to refresh its app catalog after an upgrade.

Application identity, installation/update trust and existing account/session storage are unchanged. Full Assist remains unavailable and disabled. This update does not activate native control or deploy web/backend changes. Physical installed-upgrade acceptance remains Not Verified; the published build is checked by the protected Windows release workflow.
