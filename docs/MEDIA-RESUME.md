# M4M media implementation checkpoint

Updated November 18, 2026. Starting Windows commit: 709ad9e. Companion inspected
read-only at e54af286580abbb22b4c33a905ac96034f5b6af4. Windows #11 and web #64
track this scope. No release, signing, installation or production changes.

## Current implementation

The Windows Keybinds page now records, clears, enables and resets the device-
permissions shortcut. Control+Shift+D is the default; background registration is
off by default. The native handler opens the actual permissions settings, refuses
reserved/duplicate/AltGr/plain-typing combinations, fences stale callbacks, clears
held state on view/window focus loss, suspends globals while foreground/recording,
reports conflicts and unregisters only owned shortcuts on close/quit. Global PTT
is not implemented. Mute/deafen/camera/volume actions remain unavailable until the
companion's real media adapter exists; no fake call-state controls were added.

New media-device-policy.json contains version/revision, device-global privacy and
background-shortcut policy plus bounded keybinds. Atomic writes preserve older
desktop settings and corrupt media files. Stale revisions/external changes refuse
Save. No accounts, devices, raw samples, held keys or permission grants are stored.
The camera field cannot be changed through this preference IPC; current native
camera/display denial remains authoritative. No hosted-page API was invented.

Tests: 295 app tests pass; isolated Electron UI verifies recorder, reset, clear,
background opt-in and real focused WebContents shortcut dispatch. A real native
registration/focus-suspension/cleanup fixture passes without OS key injection.
Seven settings categories and twelve screenshots were rendered; the Keybinds
screenshot was inspected. Hardware shortcut activation and actual call/camera/
microphone/RNNoise acceptance remain Not Verified. See [media controls](MEDIA-CONTROLS.md).

The prior Assist CI failure was reproduced as a temporary-directory alias in test
fixtures. ff9586b normalizes fixture roots only, preserving all production guards.
37 focused file tests and strict Clippy pass. Exact Windows CI is tracked in #1.

The first shared persistence correction prevents a preference change from
overwriting corrupt, oversized or non-object desktop settings with startup
defaults. Startup can still load safe defaults. Save reports an error and preserves
the original bytes for recovery. Valid older update/tray preferences continue to
normalize and persist using the existing atomic replacement. No session, updater
trust, account data or Assist material moves or resets.

The original persistence checkpoint passed 291 application tests, including corrupt-storage preservation,
older settings, failed atomic replacement and the existing settings error flow.
This is a persistence fix, not completion of the media feature.

The current Electron policy continues denying webcam and generic display capture.
No camera-unblock control is exposed until the real media pipeline can confirm
track shutdown and enforce policy before new capture. Permission revocation alone
cannot stop tracks already published. Existing microphone access remains narrowly
scoped; Assist protocols and protected capture are unchanged.

## Required Windows implementation

1. Define a versioned, revisioned native media-policy boundary with the companion
   owner; do not reuse Assist grants or expose arbitrary IPC. Device-global camera
   block defaults true and background hotkeys false. VoiceApi remains live-state
   authority; account/device preferences stay isolated. No active capture, PTT hold
   or reusable permission is persisted.
2. Wire policy into exact trusted frame checks and app-owned Settings. Enabling
   camera permission requires explicit local unblock and the supported web bridge;
   blocking must immediately stop publication/preview and release tracks, with
   truthful failure handling. Reload/crash/account switch invalidates live state.
3. Add validated shortcut recording and scoped actions, duplicate/reserved-key
   refusal, optional toggle-only global registration and quit cleanup. Global
   hold-to-talk stays unsupported without a separately reviewed release lifecycle.
4. Connect device/settings/diagnostics surfaces to acknowledged VoiceApi state;
   do not duplicate live controls or display fake mute/gain/device success.
5. Run fake-device browser/Electron tests, dependency audits, packaging and exact
   CI, then approved real-call/device acceptance. Personal camera/microphone
   testing has not been authorized or executed.

## Companion implementation handoff

Continue in project-palisade on main under #64, keeping the Windows repository
read-only. Inspect current VoiceApi, sdkVoice, mediaPrefs, usePushToTalkKeys,
useMicTest, MediaControls and direct-call/video integration before changing them.

Implement one effective-send model combining manual mute, deafen, PTT, server
mute/listen-only and privacy. Apply it at final processed-track publication and
replacement. Undeafen restores prior intent; focus loss, reconnect and device
switch must never leave PTT transmitting or override server restrictions.

Add real microphone/output/camera enumeration, hotplug/default fallback and
mid-call replacement/sink routing. Preserve per-participant gains. Implement
bounded actual outgoing gain/limiting and call-output gain, independently of OS
master volume. Camera block stops and releases every publication/preview path;
unblocking never starts capture.

Extend existing per-device/account mediaPrefs without overwriting established
preferences or sharing per-account device/participant choices. Specify a narrow
versioned Windows bridge proposal with request acknowledgements, revision ordering,
document/account binding, policy-vs-live-state ownership and stale-update refusal.
Browser mode must retain its safe independent preferences. Return types, fixtures,
source blobs and failure semantics before Windows implements compatibility.

Evaluate pinned Xiph RNNoise and Jitsi WASM sources, wrapper/core/model licenses,
redistribution notices, reproducible build and artifact hashes. References:
https://github.com/xiph/rnnoise and https://github.com/jitsi/rnnoise-wasm.
No dependency has been approved or integrated here. Model licensing, maintenance,
sample-rate/frame buffering, worklet CPU/latency/underruns and quality still need
verification. Package assets locally; no runtime model download or audio upload.
Publish the processed track through existing encrypted LiveKit media. Failures
fall back truthfully to standard processing while final mute/privacy gates remain.

Test actual call routing, track replacement, deafen restoration, PTT release,
camera kill-switch races, account isolation, corrupt preferences, fake-device
hotplug, processing fallback and encryption. Local microphone tests must not
publish or record samples. Separate synthetic audio measurements from real-device
and real-call acceptance. No deployment or activation is authorized.
