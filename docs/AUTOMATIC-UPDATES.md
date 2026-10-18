# Automatic update checks

Version 0.2.10 checks immediately when a packaged Windows app is ready, every six hours while running, and after a wake when the last attempt is overdue. Repeated start calls do not create duplicate timers. Concurrent checks are coalesced. Network or signature failures update status without announcing an update and retry after fifteen minutes, doubling to a six-hour cap. Shutdown clears timers/listeners and aborts pending requests.

Checks reuse the v3 GitHub release verifier: stable releases only, semantic version comparison, pinned Ed25519 trust, authenticated transition history and signed download metadata. Same or older versions are never offered. There is no legacy-signature fallback. Only metadata is fetched by a check; automatic downloads and installation are not performed.

Automatic checks notify once per available version per running process. Clicking the notification opens the existing confirmation before handing off to the manager. Manual checks show bounded plain-text release notes and let the user choose the manager or Later. The manager independently checks releases and installer integrity before installation. Device settings, tray and application menus reflect current check status.

Automated tests cover timers, retries, resume, cancellation, concurrency, authenticated offers, tampering, prereleases, downgrade suppression and manager consent. Source and packaged-archive UI tests exercise the manual check button. Real multi-hour sleep/resume behavior and authenticated installation remain operator checks.
