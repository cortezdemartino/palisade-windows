# Native media preferences and shortcuts

M4M-MEDIA-1, Windows #11. Starting SHA ff9586b; companion inspected read-only at
e54af286580abbb22b4c33a905ac96034f5b6af4. Full media implementation is unfinished.

## Delivered scope

The native Keybinds settings page records, enables, clears and resets the device-
permissions shortcut. Control+Shift+D opens the real permissions section while
the hosted view is focused. Optional Windows background registration defaults
off. Main validates and stores revisions; only the exact local controls contents,
main frame and URL may submit the closed preference operations.

The manager validates accelerators, refuses ordinary typing/AltGr/reserved
Windows/app/Assist combinations and duplicates, fences callbacks from previous
configurations, suspends globals while foreground/recording, clears held action
codes on view/window focus changes and unregisters only owned keys on destruction
or quit. Registration failures remain visible when the user returns to Settings.
No typed-text logging, OS keyboard hook or global hold-to-talk is added.

Versioned media-device-policy.json uses the existing bounded atomic writer with
revision/stale-disk validation. Corrupt/unknown state is preserved and editing
disabled rather than reset. Native storage contains no accounts, device IDs,
samples, current tracks or held-key/permission state. Camera permission is still
denied by the existing session policy. The camera field cannot be changed by the
new preference operations. No hosted media API or frozen Assist wire changed.

Only the local device-permissions action is enabled. Remaining voice/camera
actions require the companion's real VoiceApi adapter; they are shown as
unavailable rather than toggling fake live state. The handoff is recorded in
companion #64 comment 6990281890 and docs/MEDIA-RESUME.md.

## Verification

| Check | Actual outcome |
| --- | --- |
| npm run check | 295 passed, 0 skipped |
| Focused preferences/shortcut/desktop tests | Passed |
| npm run package:win | Windows x64 NSIS built, not installed |
| npm run verify:update-package | Passed; updater source and trust preserved |
| npm run verify:assist-package | Passed; normal helper remains unqualified/inactive |
| Packaged npm run test:window-controls | Passed, including all 30 permission negatives |
| Packaged Settings fixture | 13 scenario groups; seven categories and 12 screenshots |
| Focused shortcut routing | Actual Electron before-input-event on an owned fixture opens permissions |
| Native global registration | Actual Windows registration, suspension, re-registration and cleanup passed; no OS key injection |
| Visual review | Keybinds screenshot inspected; layout, recorder and controls visible |
| Focused independent source review | Stale callbacks, focus interception and subview held-key fixes verified; no remaining actionable finding in reviewed scope |

Local unpublished installer SHA-256:
7c8c55f80797b93f6e711dc0001bf3627fdaac8c0e173d4ea9abcfd09af43c14.
Version remains 0.3.6; this local build is not a published replacement release.
Exact pushed-SHA CI is linked from #11 after execution.

The earlier Assist CI failure was a test-fixture TEMP alias mismatch, reproduced
before ff9586b and fixed without relaxing native path guards. Its complete Windows
CI passed: https://github.com/cortezdemartino/palisade-windows/actions/runs/44384915630.
The fixture normalizes only owned temporary paths; production alias refusal remains.

## Unverified and blocked integration

Hardware shortcut activation, real microphone/camera access, active calls/device
switching, deafen/PTT restoration, actual gains and RNNoise processing/latency/
quality were not tested or implemented by this slice. No media samples were
captured. The Electron fixture injects keys into its own WebContents only, records
no account content and changes no persistent OS device settings.

Native live media commands and camera unblock/teardown remain blocked on a
committed, reviewed companion adapter/fixtures and corresponding Windows
integration. Permission denial alone cannot prove an active published camera
stopped. See docs/MEDIA-CONTROLS.md for the remaining inventory. This checkpoint
does not qualify full M4M, Assist, physical input or a production release.
