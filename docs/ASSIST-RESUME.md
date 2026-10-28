# Assist continuation checkpoint

Verified baseline: Windows 7afbc26be921f12ec2b722b4948a2025d7434453, exact-SHA CI40610053435 passed. This continuation adds the inactive private live channel; its final pushed SHA/CI is in Git history and Windows#1. Companion refreshed read-only at 82ac7de4ec2bbd5cc2fd48f3d15dbde6ea5fd375.

## Current result

W1 shared Quit/update-manager cleanup ownership is committed and green. W2's non-authorizing channel foundation is implemented: separate --live/--live-probe, strict private framing, bounded queues, independent reader/watchdog, safety shutdown, lifecycle-before-spawn, exact launch-nonce/channel-only completion AND actual process exit. No runtime/session/identity/input/resource command is exposed. It never kills helpers; an unproven crash retains failed ownership. This does not complete W2 or full Assist.

Local:166 app tests;72 native tests per configuration;16 real subprocess groups per configuration plus visibility; x64 NSIS/package integrity; packaged desktop/capture/14 fake-sink peer cases; audits zero vulnerabilities. Installer23034c283f58ebd2b173146f23820c578b9b7b0eece6b5431f8b7a67de52acf3, unsigned/uninstalled. Independent read-only review cleared this narrow slice. See TEST-RESULTS.md and ASSIST-LIVE-CHANNEL.md.

## Next implementation

Read ASSIST-RELEASE-PLAN.md. Complete W2 runtime atomic revoke versus retained cleanup/join, recording failures/panics. No gate lock, prompt drop, file sweep, HTTPS or join on urgent threads. Existing Prompt::drop locks before setting cancel, and SessionFiles::close may sweep/block; do not reuse either as an urgent revoker. Worker/poller join failures cannot produce success.

Before authority, retain authenticated ownership through ALL cleanup descendants. Direct guardian exit/channel-only proof is insufficient. Consider a non-killing/no-breakaway job with exact retained handle/assignment before authority; query actual ActiveProcesses==0 after closing admission, never rely solely on job signaling/completion notifications or process-name polling. Close the crash-before-child-report race. Current channel deliberately refuses broader cleanup scope. NativeStop/session flags must be connected to the same revocation owner.

Keep SessionGate/LeasePoller as one registry; grant offers do not recreate runtime or restore revoked grants. Then W3 source/consent/owned-answer signing and W4 fresh input generations/readiness. Current monitor geometry cannot silently stand in for window-only input; keyboard desktop scope needs truthful consent. Transport setup precedes DTLS: require current source/enrollment/consent/covering lease without temporarily setting peer_verified. Await native observed-peer readiness/idle warmup before packets.

Sensitive UI must revoke/fence/reap/release before approvals, block new controllers/ACK/signing/warmup throughout, and require fresh input afterward. Cover capture/enrollment/settings/clipboard/files/save. Stop/Deny remain usable. Keep independent resources. App-owned drops need per-file local confirmation before any content read; hosted page must see no file objects/bytes during any drop phase. isTrusted and remote/cleanup-only UP events are not host consent.

## Companion and gates

Companion remains read-only. Docs/50 at82ac7de now supplies initial text/file channels, local actions, browser-owned helper authority, relay route, track names, safety capacity and cleanup events. Integrate these; do not ask Claude to recreate them. Its images/sync/resume/drop remain unsupported and open requirements. Updated ASSIST-COMPANION-HANDOFF-PROMPT.md covers remaining full features, keyboard/annotations/reacquisition, stale SCTP input, relay and joint acceptance.

Official docs/42 uses equivalent exact-candidate LOCAL full verification while companion Actions are owner-disabled. CI absence alone is not a blocker. Windows CI remains enabled and requires the exact pushed SHA. Production issuer is project#47; disposable-device/native/joint acceptance is Windows#5, with no test-device answer received.

Architecture and reviewer authorization already exist. Installation, native activation and real input on approved disposable devices require separate approval; never use the personal desktop. Publication/production remain separately gated. No private production keys, installation or activation occurred. Keep this existing main checkout, profile/session/updater identity and stopped v0.3.0 tag. Window persistence stays queued after Assist readiness.
