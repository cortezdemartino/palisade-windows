# Palisade Desktop v0.2.5

- Register the existing Palisade Windows notification identity with the Palisade display name and a shell-readable logo before sending notifications. Development Electron runs do not rewrite the production identity.
- Package the notification logo outside the application archive so Windows can read it directly.
- Request native Windows taskbar attention flashing for new unread messages while Palisade is unfocused. Stop flashing when the app receives focus or unread messages clear. Initial unread synchronization remains silent, and toast throttling does not suppress taskbar attention. Windows controls the attention color.

Automated identity/attention tests and packaged asset checks cover these changes. The visible notification header, physical taskbar flashing and real-message delivery still require operator confirmation after upgrading. Signing trust, installation identity, sessions and the persistent browser partition are unchanged.
