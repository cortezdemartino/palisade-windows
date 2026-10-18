# Palisade Desktop v0.2.4

- Check signed stable releases at startup, every six hours and after an overdue wake, with retry backoff and clean shutdown cancellation.
- Add manual Check for Updates actions, live status and release-note prompts. Downloading and installation still require the existing update manager and user confirmation.
- Add a red ring around the taskbar logo for unread chats and enlarge the normal logo artwork within the Windows icon slot.
- Convert increases in existing chat unread labels into generic native notifications without message text or account data. Preserve browser isolation and expose no page-accessible desktop bridge.
- Show notification delivery status in device settings and retain Windows notification/privacy controls.

Unread observation follows the hosted UI’s visible unread totals, not a separate Matrix connection. Initial synchronization is silent; messages immediately marked read and simultaneous offsetting count changes may not produce alerts. Windows settings govern banner delivery and pinned shortcut artwork may be cached.

Automated and packaged fixture tests do not establish real-account message delivery, physical taskbar rendering or authenticated-session installation success. Those checks require operator validation. The installation identity, browser partition and update signing root are unchanged.
