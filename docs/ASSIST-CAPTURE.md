# Isolated Windows capture

Status: **Experimental, inactive**. This component has passed app-owned Windows fixtures. It is not connected to the hosted application or qualified for cross-client encrypted screen sharing.

The hosted session continues to deny generic display capture. The internal capture coordinator reuses the existing local source picker, requires a visible trusted main window, rechecks the selected source, and creates a separate sandboxed provider in an ephemeral partition. No source is selected automatically. The provider receives only the exact selected source, with audio disabled.

An app-owned peer in the provider sends a single video track to an app-owned isolated preload in the hosted window. No SDP, ICE credentials, certificate, filesystem path or native method is exposed to the page. The selected incoming stream is available through an owned video element's `srcObject`, which a future canonical adapter can publish through the existing MatrixRTC/LiveKit encrypted media path. This local DTLS hop does not establish or replace cross-client media encryption.

Both peers verify the actual remote certificate against the other owned peer's fingerprint. Only IPv4 UDP host candidates are retained. The native guardian checks that each bound socket belongs to the authenticated Electron creator or a same-user/session descendant, retaining process handles and checking ancestry creation times. Candidate and selected-route checks reject unknown, foreign, ambiguous or closed bindings. These checks do not establish an operating-system firewall or a universal no-egress guarantee.

Fresh challenges use monotonic deadlines. Only one challenge is outstanding per peer, and delayed inspection cannot extend the original deadline. Main independently destroys the provider if a peer freezes, crashes, navigates, disappears or fails validation. The provider also expires without main heartbeats. Cancellation before readiness cannot revive sharing. Closing the selected fixture window ends capture and clears source state. Closing or hiding the owner stops the operation. Repeated operations use distinct state and nonces; stale replies cannot stop a newer operation.

## Reproducible checks

Run `npm run build:assist`, then `npm run test:assist-capture`. The test runner uses Electron 44.7.2 with the release file-protocol restrictions and requires fresh result files from each subprocess. After packaging, set `PALISADE_WINDOW_SMOKE_ASAR` to the absolute `out/win-unpacked/resources/app.asar` path and repeat the command to test the packaged modules and preloads.

The fixtures exercise:

* A synthetic canvas crossing isolated worlds through a video `srcObject`.
* Actual DTLS video, certificate checks, native socket ownership and decoded pixels.
* The existing source picker with one app-created window, covered by another app-created window. The received pixels must match the chosen window, and closing it must stop capture.
* Hosted legacy capture rejection, including while the isolated provider is active.
* Disabled mode, foreign sockets, early cancellation, wrong certificates, hostile DOM changes, repeated capture, cloned-track teardown, reload, crash and frozen peers.
* Slow ownership checks combined with wall-clock rollback and a frozen consumer, bounded by the original monotonic deadline.

The fixtures do not capture personal applications or entire monitors. Monitor enumeration, mixed DPI, display removal, secure-desktop transitions and actual MatrixRTC publication still require disposable-device and cross-client acceptance. The native visibility fixture separately verifies required owned controls under a hidden process startup; it does not qualify physical emergency-stop or input operation.

## Integration gate

The canonical web handoff must finalize capture start, answer, cancellation, status and lifecycle schemas before a page-facing adapter is added. Stream/source operations must be bound to the exact Assist session and LiveKit track SID. Ending child Assist permissions must not end the parent group stream. Production capture remains disabled until this adapter, encrypted publication and local stop behavior pass review and acceptance.
