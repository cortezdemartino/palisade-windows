# Full Assist integration and release plan

Starting Windows commit: `1234a184dcb76b7dbc81615e865ebfb8ed22f7bf`, with successful exact-SHA CI. Starting companion reference: `c0778935a6a08303bf7a9d7de8f2fa75aa8ef536`. Work stays on Windows main. Companion source stays read-only here; Claude owns that repository's changes.

The target is an enabled, tested release candidate. Passing component checks does not complete the product. Installation on test devices, native activation, production deployment and release publication retain their explicit owner approval requirements. Architecture approval under project-palisade#33 is already recorded.

## 1. Finish independent Windows resource handling

Implement a session-owned adapter around the existing native file store. One runtime owns one quota/store, immutable session/grant/direction bindings and cancellation for pending dialogs and active handles. Never hold the authority gate during dialogs or disk I/O. Cancellation flags remain available even while the store is busy. Check current native authorization before and after each sensitive operation and discard late results. Receive permission alone must not accept an individual file; use trusted local confirmation of the exact offer before the save dialog. No path-taking IPC or arbitrary filesystem API.

Tests use signed fixtures and temporary NTFS files. Cover cross-session/token/direction swaps, grant expiry, pending dialog cancellation, stalled-operation cleanup, final commit revocation, independent directions, input-only suspension and quota retention. No personal files/clipboard or real input. Obtain independent component review, run native/app checks and package verification, then commit and push.

Status: **Implemented, inactive**, with local packaging/verification passed and exact-SHA CI recorded after push. Session-owned requests, separate directional stores with one atomic quota, cancellation/cleanup watcher and per-file native confirmation are implemented. Ten new signed-fixture/temporary NTFS cases pass within the 52-test native suite; confirmation is simulated in tests, so real consent UI remains Not Verified.

## 2. Integrate Claude's committed handoff

Record the exact companion SHA and required High-risk CI results. Inspect the authority fixes and canonical types/fixtures rather than trusting a completion message. Verify fail-closed endpoint/session leases and post-await request revalidation. Reconcile enrollment/issuer trust, native input peer certificate ownership, signaling answer/cancel/status and capture publication/lifecycle.

Implement the matching Windows bridge, owned remote gateway, native coordinator/IPC, resource adapters and local host controls in verified slices. Preserve the three-member description bridge, sandbox, existing web E2EE, updater trust and persistent session. Expose only actually negotiated capabilities. Bind capture/source operations to the exact session and LiveKit stream incarnation. Ending child Assist must preserve the group stream/voice. Stream end must revoke all related child permissions. Unsupported images/resume/drag-drop remain unavailable rather than silently falling back.

The current native diagnostic channel remains v1 with 16KiB frames. Resource wiring must account for the canonical data peer's larger chunks through bounded splitting or a separately reviewed typed channel; do not silently broaden diagnostic v1 or expose a generic invoke.

The publisher's endpoint lease does not authorize a helper's local resources. Implement only the role whose enrollment, device binding and local authority are established by the committed contract. Never reuse another endpoint or extract Matrix credentials.

Status: **In progress against unchanged docs/49 native bridge v1**. Companion ae89e414ec71a193094ef7cbee5ad678418ff6f6 now implements the web adapter, shared-certificate input peer, encrypted capture publication and virtual keyboard, browser-tested with a mock endpoint. Windows has the matching inactive bridge, statement separation and qualified issuer-record policy, with independent component review. Its live coordinator/enrollment tuple/owned remote peer/capture and native IPC wiring remain unfinished. Resource data plane/local actions/helper authority, relay configuration and cleanup semantics need the follow-up recorded in Windows#4 comment 6433593555. Production issuer setup is tracked in project-palisade#47. Read-only refreshes continue; official companion CI evidence remains required. The current [Claude prompt](ASSIST-COMPANION-HANDOFF-PROMPT.md) lists only remaining integration work.

## 3. Execute real device acceptance

Use disposable Windows devices/accounts under Windows#5 after explicit installation/activation approval. Record Windows/web SHAs, package digest, OS, hardware, display/DPI/layout and transport conditions. Do not test remote input on the owner's sensitive personal desktop.

Required matrix: Windows host/Windows guest, Windows host/web guest, web host/Windows guest for supported native-host scenarios, and browser regression. Prove actual selected-window/monitor encrypted viewing; pointer/click/drag/scroll; keyboard/modifiers/permitted shortcuts; independent clipboard directions; accepted file send/save/hash/MOTW/no-overwrite/cancel; source loss; original deadline and revocation; stream end while group voice continues.

Qualify the local emergency stop before broader control tests. Test frozen renderer/network, guardian/child termination, held input release, stale packets, lock/sleep/session switch, UAC/UIPI/protected desktops, multiple monitors/DPI and reconnect without restored control. Test approved NAT/TURN paths and encryption with an unauthorized observer. Record measured performance and unexecuted cases honestly. A failure returns the affected slice to implementation/review.

Status: **Not Verified**, requires test-device access and owner approval. Local process/window fixtures do not substitute for these tests.

## 4. Build and verify the release candidate

Use a fresh version/tag; do not move or reuse the stopped v0.3.0 tag. Run official High-risk verification and exact-SHA Windows CI, native/production audits, NSIS build, package integrity/fuses, updater signature/hash/rollback tests and actual installed-upgrade/session-preservation acceptance. Verify removal/upgrade cleanup of the native endpoint. Keep private keys outside source, client and artifacts.

The live quit/update path must revoke Assist first and wait for the native child to finish owned-input cleanup and exit before starting an installer that replaces its image. A protected-desktop or stalled-cleanup condition must postpone installation rather than kill the last cleanup holder or bypass verification. Test update requests during an active session and pending transfer, including failure and cancellation. This lifecycle hook is part of stage 2 integration and stage 3 acceptance.

Prepare a concrete candidate with release notes, asset hashes, tested compatibility, complete results and remaining limitations. Seek the separate release/production approval only after required evidence is complete. After approval, use the existing protected signing and GitHub Releases workflow, verify published metadata/assets and the approved rollout. Stop on any verification/signing failure.

Status: **Blocked by stages 2 and 3**. No partial disabled Assist release substitutes for this candidate.

## 5. Close out

Update the implementation report, practical user/administrator guides, parent project-palisade#18/#33 and Windows#1/#3/#4/#5 review links with exact revisions and outcomes. Print the current companion prompt if integration is still externally blocked. Keep every incomplete stage open. Address the previously requested window-size persistence bug after Assist testing readiness.
