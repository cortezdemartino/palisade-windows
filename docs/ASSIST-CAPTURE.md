# Isolated Windows capture

Status: **Experimental, inactive**. This component has passed app-owned Windows fixtures. It is not connected to the hosted application or qualified for cross-client encrypted screen sharing.

The hosted session continues to deny generic display capture. The internal capture coordinator reuses the existing local source picker, requires a visible trusted main window, rechecks the selected source, and creates a separate sandboxed provider in an ephemeral partition. No source is selected automatically. The provider receives only the exact selected source, with audio disabled.

An app-owned peer in the provider sends a single video track to an app-owned isolated preload in the hosted window. The internal local hop exposes no SDP, ICE credentials, certificate or filesystem path to the page. The selected incoming stream is available through an owned video element's `srcObject`. The inactive native v1 adapter now labels it with the canonical data-palisade-assist-capture operation attribute for the companion's existing MatrixRTC/LiveKit publication adapter. This local DTLS hop does not establish or replace cross-client media encryption.

Both peers verify the actual remote certificate against the other owned peer's fingerprint. Only IPv4 UDP host candidates are retained. The native guardian checks that each bound socket belongs to the authenticated Electron creator or a same-user/session descendant, retaining process handles and checking ancestry creation times. Candidate and selected-route checks reject unknown, foreign, ambiguous or closed bindings. These checks do not establish an operating-system firewall or a universal no-egress guarantee.

Fresh challenges use monotonic deadlines. Only one challenge is outstanding per peer, and delayed inspection cannot extend the original deadline. Main independently destroys the provider if a peer freezes, crashes, navigates, disappears or fails validation. The provider also expires without main heartbeats. Cancellation before readiness cannot revive sharing. Closing the selected fixture window ends capture and clears source state. Closing or hiding the owner stops the operation. Repeated operations use distinct state and nonces; stale replies cannot stop a newer operation.

The capture-publication component implements canonical capture.start/bind/cancel/stop and status/events through the existing strict native bridge. It binds the native source to an operation and room, accepts one tentative track SID, rejects rebinds, and invalidates matching child authority before source teardown. It owns no input/resource authority itself. The live coordinator must provide synchronous child revocation and independent fail-closed cleanup; production wiring is still unfinished. An old source/provider load failure, crash or queued timer cannot stop its replacement.

The track SID is a page claim. A capture.bind acknowledgement is not independent proof that the selected source produced the published track. Native grants still require the native enrollment tuple, exact source/room/stream binding, independent service verification, a fresh lease and native local consent. A window share cannot silently authorize desktop-wide input. Ending child Assist must leave its parent group stream intact.

The hosted preload is assembled at build time from the maintained notification/native bridge preload and local-video preload. The generated file is ignored by Git and rebuilt before packaging and fixture runs. Production continues to use the original preload without native activation. Fixtures use the assembled preload under the same sandbox and context isolation.

Route checks read the live ICE transport's selected candidate pair rather than an asynchronous statistics snapshot. Chromium's exact `redacted-ip.invalid` marker is retained as unknown address metadata. It is accepted only with independently proven native IPv4 bindings, reciprocal selected ports, both actual owned certificates and fresh socket ownership checks. No other placeholder, missing address, relay, TCP or IPv6 fallback is accepted by this local hop. Specific interface bindings are supported alongside wildcard/loopback bindings. The marker cannot prove the path taken by packets; these checks authenticate the owned encrypted endpoints.

## Reproducible checks

Run `npm run build:assist`, then `npm run test:assist-capture`. The test runner uses Electron 44.7.2 with the release file-protocol restrictions and requires fresh result files from each subprocess. After packaging, set `PALISADE_WINDOW_SMOKE_ASAR` to the absolute `out/win-unpacked/resources/app.asar` path and repeat the command to test the packaged modules and preloads.

The fixtures exercise:

* A synthetic canvas crossing isolated worlds through a video `srcObject`.
* Actual DTLS video, certificate checks, native socket ownership and decoded pixels.
* The existing source picker with one app-created window, covered by another app-created window. The received pixels must match the chosen window, and closing it must stop capture.
* The full canonical native bridge path with that app-owned window: negotiate, pick, read the operation-labelled srcObject, bind a synthetic SID once, reject a second bind and observe source-stop. This does not publish to LiveKit.
* Hosted legacy capture rejection, including while the isolated provider is active.
* Disabled mode, foreign sockets, early cancellation, wrong certificates, hostile DOM changes, repeated capture, cloned-track teardown, reload, crash and frozen peers.
* Slow ownership checks combined with wall-clock rollback and a frozen consumer, bounded by the original monotonic deadline.

The fixtures do not capture personal applications or entire monitors. Monitor enumeration, mixed DPI, display removal, secure-desktop transitions and actual MatrixRTC publication still require disposable-device and cross-client acceptance. The native visibility fixture separately verifies required owned controls under a hidden process startup; it does not qualify physical emergency-stop or input operation.

## Integration gate

The canonical docs/49 handoff and companion publication adapter exist. The matching capture component passes an actual Electron/window fixture, while live native coordinator/enrollment/source authorization wiring and real encrypted cross-client publication remain incomplete. Production capture remains disabled until integration, encrypted publication and independent local stop pass review and disposable-device acceptance.
