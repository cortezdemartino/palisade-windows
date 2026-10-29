# Assist continuation checkpoint

Verified baseline: Windows 9f529087653d4b0259d4c0749bc9c8dd7f692978, exact-SHA CI40787475543 passed. This continuation adds dormant atomic revocation, retained cleanup and independent consent; its final pushed SHA/CI is in Git history and Windows#1. Companion refreshed read-only at 82ac7de4ec2bbd5cc2fd48f3d15dbde6ea5fd375.

## Current result

W1 shared Quit/update-manager cleanup ownership is committed and green. W2's non-authorizing channel foundation is implemented: separate --live/--live-probe, strict private framing, bounded queues, independent reader/watchdog, safety shutdown, lifecycle-before-spawn, exact launch-nonce/channel-only completion AND actual process exit. No runtime/session/identity/input/resource command is exposed. It never kills helpers; an unproven crash retains failed ownership. This does not complete W2 or full Assist.

Native internal APIs now separate prepared atomic RuntimeRevoker/FileRevoker from blocking retained cleanup. Prompt/poller/input/file joins report sticky failures; failed owners remain retained, and panic/destructor/payload failure does not masquerade as success. Native prompts cancel before gate waits and track global/input-only flags. Exact own-grant consent survives unrelated input suspension. Files close without blocking registry/I/O locks; wait_cleanup proves both stores/selection/watcher completion. The release helper uses Rust unwinding and a fixed synthetic cleanup probe. None of this attaches the runtime to the channel or provides full-lifetime sensitive-UI guarding.

Local:168 app tests;85 native tests per configuration;17 real subprocess groups per configuration plus visibility; x64 NSIS/package integrity and actual packaged cleanup probe; packaged desktop/capture/14 fake-sink peer cases; audits zero vulnerabilities. Installer80607651456da6c5fd3d6858282685cad48a9b40414361cd88b43232876abc48, unsigned/uninstalled. Helper264a40d252544583800baf4360fcec0bebf94c68ecac2ba65760b30aeb15596a matches built/packaged bytes. Initial EBUSY/unread timing failures and corrected reruns are recorded. Independent review cleared this inactive slice. See TEST-RESULTS.md and ASSIST-LIVE-CHANNEL.md.

## Next implementation

Read ASSIST-RELEASE-PLAN.md. Finish W2 authenticated descendant ownership and live runtime integration using the implemented atomic revokers and retained join APIs. No gate lock, prompt drop, file sweep, HTTPS or join on urgent threads. Preserve the exact-grant resource consent fix and sticky worker failures. Do not treat native Drop/is_finished, channel-only proof or a direct guardian exit as active-session completion. Release abort/foreign/FFI and combined host failures remain outside Rust unwinding recovery.

Before authority, retain authenticated ownership through ALL cleanup descendants. Direct guardian exit/channel-only proof is insufficient. Consider a non-killing/no-breakaway job with exact retained handle/assignment before authority; query actual ActiveProcesses==0 after closing admission, never rely solely on job signaling/completion notifications or process-name polling. Close the crash-before-child-report race. Current channel deliberately refuses broader cleanup scope. NativeStop/session flags must be connected to the same revocation owner.

Keep SessionGate/LeasePoller as one registry; grant offers do not recreate runtime or restore revoked grants. Then W3 source/consent/owned-answer signing and W4 fresh input generations/readiness. Current monitor geometry cannot silently stand in for window-only input; keyboard desktop scope needs truthful consent. Transport setup precedes DTLS: require current source/enrollment/consent/covering lease without temporarily setting peer_verified. Await native observed-peer readiness/idle warmup before packets.

Sensitive UI must revoke/fence/reap/release before approvals, block new controllers/ACK/signing/warmup throughout, and require fresh input afterward. Cover capture/enrollment/settings/clipboard/files/save. Stop/Deny remain usable. Keep independent resources. App-owned drops need per-file local confirmation before any content read; hosted page must see no file objects/bytes during any drop phase. isTrusted and remote/cleanup-only UP events are not host consent.

## Companion and gates

Companion remains read-only. Docs/50 at82ac7de now supplies initial text/file channels, local actions, browser-owned helper authority, relay route, track names, safety capacity and cleanup events. Integrate these; do not ask Claude to recreate them. Its images/sync/resume/drop remain unsupported and open requirements. Updated ASSIST-COMPANION-HANDOFF-PROMPT.md covers remaining full features, keyboard/annotations/reacquisition, stale SCTP input, relay and joint acceptance.

Official docs/42 uses equivalent exact-candidate LOCAL full verification while companion Actions are owner-disabled. CI absence alone is not a blocker. Windows CI remains enabled and requires the exact pushed SHA. Production issuer is project#47; disposable-device/native/joint acceptance is Windows#5, with no test-device answer received.

Architecture and reviewer authorization already exist. Installation, native activation and real input on approved disposable devices require separate approval; never use the personal desktop. Publication/production remain separately gated. No private production keys, installation or activation occurred. Keep this existing main checkout, profile/session/updater identity and stopped v0.3.0 tag. Window persistence stays queued after Assist readiness.
