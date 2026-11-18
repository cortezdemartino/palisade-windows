# Companion Assist integration handoff

Windows is the sole writable repository for this assignment. Companion main was
fetched and inspected read-only at `84b379f730d13490dad2417b30065442152f0035`.
Do not change frozen native/control/extension sources or fixtures to solve these
integration issues. No production activation, deployment or release is requested.

## Corrections received

Companion commit `af21c9f` implements the earlier stale-loss and scoped-Stop
requests. Source inspection confirms intentional peer retirement without an
unscoped bye, callbacks bound to the grants the old peer carried, exact grant ID
and generation checked synchronously at service mutation, and input-only
`peer.close {session}` instead of global native Stop. A helper control-channel
close preserves independently open resource channels. Ambiguous old byes and
lifecycle reports refresh rather than revoke a successor.

Companion docs/54 section 13 records its own automated and browser evidence.
Those runs were not executed here and do not qualify the real Windows endpoint.
Do not request these same corrections again. Regress their behavior when testing
the combined exact candidates, including old callbacks and delayed revoke requests
before and after successor activation.

## Remaining companion engineering

Reconcile transport capabilities when a newly approved resource direction needs
a channel absent from the current connection. `syncNative` retains an existing
open peer while `NativeInputPeer` fixes its resource list at construction. A
clipboard-only peer cannot gain a file channel by updating a grant object.
Use a validated channel addition or deliberate replacement. Preserve exact peer
incarnations, independent input/resource authority, current signed permissions,
transfer cancellation and explicitly retained interrupted staging. Never pre-open
an unauthorized channel or silently restart ended input.

Test clipboard-first, file-first, late direction approval, revocation and active
transfer replacement. Also test first input after a resource-only connection:
Windows retires that exact resource transport only after verified native input
acknowledgement; the helper must construct the matching fresh input connection.
No change to the frozen page/control wire is needed for channel reconciliation.

## Windows continuation and joint qualification

Windows now connects resource-only/shared authenticated DTLS, native-only ICE,
private text requests, per-direction native consent and an exact one-shot outbound
packet check. Plain-text clipboard is qualification-only; images, file dispatch,
synchronization, drag/drop and resume remain separate Windows engineering work.
Private file staging and verification do not constitute an end-to-end transfer.

Use docs/52 and docs/54 with an approved disposable Windows device, test accounts,
isolated services and independently supplied public issuer fingerprint. The current
Windows qualification builder defaults to capture-only. Explicit `--native-input`
and `--clipboard-text` options are for independently approved device qualification;
its report attests compiled features and hashes, never successful acceptance.
Do not install/start a candidate on a personal desktop or fabricate native consent.
A dedicated enrollment stage must remain separate from decoded-viewing WIN-P01.

The original 30 acceptance cases, all nine fixture families and six frozen source
blobs remain unchanged. Manifest SHA-256:
`f9ac9d835333e341907ae63f2d2a497ef4944607ea60fc344e81ad64115606b0`.
No actual Windows cross-client case has passed in this campaign. Production
issuer/pin #47, TURN #48, Firefox #49, device acceptance Windows #5, integrated
review #33 and exact-candidate release approval remain independent gates.

## Seamless cooperative control

The new immediate handoff requirement cannot be implemented by changing frozen
v3's 2,000-ms rule. Native delivery arbitration remains Blocked: observation does
not prove a helper UP cannot affect a concurrent host hold. Keep localYield false.
A reviewed safe mechanism and separately negotiated successor with new fixtures
are required before implementing immediate handoff. Do not invent a temporary
wire protocol, install a driver or count fake input as physical acceptance.

Return exact commits, test commands, evidence classifications and any required
contract adjustment to Windows #4. Keep every unexecuted device case Not Verified.
