# Assist continuation checkpoint

Verified baseline: Windows `7fcae472c041b1569233880cff96ae0a691bc7d1`, CI run 40602184816 passed; companion `ae89e414ec71a193094ef7cbee5ad678418ff6f6` inspected read-only. Current changes implement the full feature ledger/W1-W10 plan and W1 application exit/update ownership; their exact commit/CI evidence is recorded in TEST-RESULTS.md and Windows#1 after push.

## Required scope

Read ASSIST-RELEASE-PLAN.md first. Every required feature remains in scope, including image clipboard, fresh-authorized resume, drag/drop, full keyboard/virtual keyboard, pointers/annotations and all client roles. Component tests or disabled code do not complete the feature. No known Windows work may remain when declaring Windows integration-ready.

## Next implementation

Proceed with W2 live typed native IPC/coordinator, preserving diagnostic v1. Existing diagnostic kill/exit behavior cannot own active cleanup. Register actual child-exit/release holders with the implemented application lifecycle BEFORE admitting native authority. Use SessionGate/LeasePoller as one registry. Then W3 source/consent/owned-answer signing and W4 input generation/cleanup/readiness, while Claude finalizes resource/relay/reacquisition contracts.

W1 now gates both app quit and manager copy/spawn, retains failed/pending holders, reports pending status and preserves profile storage. Its real Electron quit test uses a FAKE holder. No real native cleanup is claimed. See native architecture and current evidence.

Critical native UI requirement: fence/reap/release input before enabling approval controls, guard the entire consent/source/settings/file UI lifetime, block replacement controllers/ACK/transport setup/warmup, and require fresh authority afterward. Preserve independent resources. External drops use an app-owned surface and separate per-file local confirmation before content reads; the hosted page must not observe file objects/bytes in any drag/drop phase. Never use isTrusted as host consent.

## Dependencies and approvals

Companion remains read-only; use ASSIST-COMPANION-HANDOFF-PROMPT.md for Claude. Its official docs/42 policy replaces owner-disabled Actions with equivalent exact-candidate LOCAL full verification; missing companion CI alone is not a blocker. Windows CI remains enabled. Production issuer setup is project#47. Native/device/joint acceptance is Windows#5; no test-device answer has been received.

Architecture and independent reviewer authorization already exist. Installation, native activation and real-input testing require separate approval on disposable devices. No input on the owner's sensitive desktop. Production and release publication remain separately gated. No private production signing material, installation or activation occurred. Preserve the existing main checkout, session/profile/updater identity and stopped `v0.3.0` tag. Window persistence remains queued after Assist readiness.
