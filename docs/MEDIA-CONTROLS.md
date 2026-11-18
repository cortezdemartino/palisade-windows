# Windows media controls

Updated November 18, 2026. M4M-MEDIA-1 is **unfinished**. Windows #11 and companion
web #64 track delivery. Companion e54af286580abbb22b4c33a905ac96034f5b6af4 was
inspected read-only. No deployment, signing, release or camera/microphone test on
personal hardware was performed.

## Available Windows shortcut

Open Settings > Keybinds. Control+Shift+D opens device permissions while the
Palisade page is focused. Choose Record shortcut, press Control or Alt with a
supported key, then release it. Escape cancels; Tab leaves the recorder. Clear
removes the binding; Reset restores the default and turns background shortcuts off.
Changes save immediately on this PC. Reserved Windows/app/Assist combinations,
duplicates, plain typing and AltGr character combinations are refused.

Background shortcuts are optional and off by default. When enabled, Windows
registers only supported native actions. The current supported action opens
device permissions. Foreground uses the app-scoped handler; owned global keys are
unregistered while Palisade has focus, so they do not steal recorder input. A
registration conflict is reported rather than silently claiming success. Closing
the owning window or quitting unregisters owned keys. No broad keyboard hook,
typed-text log or global hold-to-talk implementation is introduced.

Mute, deafen, camera, volume and PTT shortcuts are unavailable in this native
panel until they can operate the real VoiceApi and acknowledge the resulting
state. Existing hosted call controls remain in use. Camera access remains blocked;
the new settings cannot turn it on or bypass protected Assist capture.

## Storage and boundaries

media-device-policy.json under the existing per-user application directory uses
schema version 1, safe-integer revision and a 16-KiB read bound. It contains only
device-global policy and validated shortcut assignments. Camera block defaults
true; background shortcuts default false. Account-specific media preferences remain
in the hosted app's existing store. Active tracks, camera transmission, PTT holds,
temporary grants, audio samples, accounts and credentials are never written here.

Updates use the existing atomic fsync/replace helper. Invalid/future/corrupt files
remain intact; startup uses safe defaults and disables editing until recovery.
Save refuses stale revisions and changed on-disk state. Save errors do not advance
the in-memory revision or disclose profile paths. Restore a verified backup or
correct the file with Palisade closed; do not delete account/session data.

The preference IPC exists only in the app-owned controls preload. Main requires
the exact controls contents, main frame and fixed local URL, then accepts only
closed background/binding/reset operations. Hosted pages and subframes cannot
change policy or register keys. No media protocol or Assist fixture changed.

## Feature inventory and next integration

| Capability | Windows status | Remaining work |
| --- | --- | --- |
| Native key recorder, persistence, background opt-in | Implemented and isolated-UI/native-registration verified | Hardware press acceptance; remaining media actions need VoiceApi adapter |
| Camera hard block | Existing native denial preserved | Coordinated track/preview termination and acknowledged capture policy before any unblock |
| Mic/speaker/camera selection | Existing web VoiceApi owns devices | Real Windows settings/status integration with committed media contract |
| Mute/deafen/PTT and app gains | Existing web implementation; new integration Not Verified | Final outgoing gate, restore intent, actual track/sink/gain and reconnect tests |
| RNNoise enhancements and mic test | Not implemented here | Web AudioWorklet/pinned WASM/model review, real encrypted uplink and performance evidence |
| Account-specific preferences | Existing web store preserved | Account-isolated migration and revisioned synchronization |
| Real calls/device hotplug/media quality | Not Verified | Approved devices/test services, real-call acceptance and measured results |

The companion must commit the narrow typed media adapter and fixtures before
Windows can integrate live state. Required boundaries: document/account binding,
version negotiation, revision ordering, correlated command acknowledgements,
actual effective state, disconnection/timeout behavior and local camera-block
enforcement. Native policy must remain authoritative. A page's Boolean must not
be accepted as proof that an unresponsive capture has terminated. Preserve existing
VoiceApi/LiveKit/MatrixRTC and account preferences; do not invent a second engine.
The exact assignment is in [MEDIA-RESUME](MEDIA-RESUME.md).

## RNNoise evaluation checkpoint

No package was added. Xiph's source identifies BSD-3-Clause; Jitsi's wrapper
identifies Apache-2.0. Licensing review must separately cover the exact embedded
core and model. The [Jitsi README](https://github.com/jitsi/rnnoise-wasm/blob/master/README.md)
states its async and sync prebuilds currently use different RNNoise generations;
the [wrapper build](https://github.com/jitsi/rnnoise-wasm/blob/master/build.sh)
also replaces those prebuilds. Choose and hash the actual AudioWorklet artifact,
not just a package version. [Xiph's README](https://github.com/xiph/rnnoise/blob/main/README)
notes model downloads during source preparation. This must not become a runtime
download in the client. Source/model/build provenance, maintenance/security audit,
CPU, latency, underruns and speech quality remain unverified. No Krisp-equivalence
or real-call enhancement claim is made.

## Verification

295 application tests pass, including schema/revision/corruption, reserved-key,
stale callback, focus, PTT refusal, conflict feedback and IPC isolation checks.
The Electron fixture exercises seven rendered categories, recorder/clear/reset,
focus dispatch, actual Windows registration and owned cleanup. It injects keys
only into its own WebContents, never the OS or a personal app. Settings layouts
include a 640x440 window, text scaling, emulated DPI, forced colors, reduced
motion and accessibility tree checks. Real OS DPI, Narrator and hardware-key
activation are not inferred from those fixtures. Packaging and exact-SHA CI are
recorded with the committed candidate in #11.
