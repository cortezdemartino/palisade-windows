# Project Palisade: implement stream-bound group assistance

Writable repository: cortezdemartino/project-palisade. Inspect cortezdemartino/palisade-windows read-only. Work on main, preserve concurrent changes, and create no branches or pull requests. Do not deploy, enable production features, publish Windows releases or implement/install Windows native input.

Parent enhancement: project-palisade#18. Companion dependency: palisade-windows#4. Native endpoint review: project-palisade#33.

## Audit first

Refresh main and record the exact revisions. The last inspected published web revision is ee9c2763f19e62b405bbf9a9c97f187bc5a1291a. It now contains docs/47, control v1 types/fixtures/service grants, Olm signaling, peer data/transfer components and input helpers. Its authority is still pairwise and capped at fifteen minutes. Reuse that work; do not repeat the completed v1 description bridge or create competing data protocols. Windows still reports capture unavailable and native endpoint absent.

Read repository instructions, the risk-based testing policy, docs/44 through docs/47, services/admin/src/{assist,assistControl,calls,media,voiceAuthz}.ts, apps/web/src/platform/desktopAssistBridge.ts, apps/web/src/assist/control/, the Assist feature modules, directCallController.ts, sdkVoice.ts, assistSignal.ts and annotations.ts. Read the Windows ASSIST-STREAM-SESSIONS.md, ASSIST-CONTROL-HANDOFF.md, ASSIST-ENDPOINT-REVIEW.md and ASSIST-THREAT-MODEL.md at the exact Windows revision you record.

## Owner-approved amendment and first deliverable

The owner approved the native architecture and requested: any eligible stream viewer, including in a group, can request to assist; the streamer can ask the group or a specific person for help; permissions and duration are set for the session up to two hours; when the stream ends, every associated permission ends. This supersedes the earlier future-product fifteen-minute/pairwise requirement. Owner approval is recorded in project-palisade#33; independent review still applies.

Implement an explicitly versioned stream/group contract and real service validation. Preserve the description-only bridge v1 and legacy control v1's existing validation/fixtures. Do not merely replace fifteen minutes with two hours inside the old schema.

1. Viewer-initiated requests and streamer-initiated group solicitations or specific-user invitations, bound to authenticated stream eligibility and normal room/call access rules. A request can propose permissions/time; the streamer makes the final decision.
2. A group solicitation grants nothing automatically. Bind acceptance to the selected helper and exact device; resolve concurrent responders atomically with one native controller at a time. Replacing a controller invalidates the old one before new input can act. Bound/deduplicate/rate-limit requests and expire them when the stream ends.
3. Separate viewing, pointers/annotations, mouse, keyboard, clipboard directions and file directions. Keep independently verified device/endpoint proof, local host consent and native acknowledgment. Group roles, media tokens and renderer booleans cannot grant native authority. Review docs/47's residual identity/attestation risks and do not export Matrix secrets or reuse updater keys.
4. For the new model, a positive host-selected duration up to two hours from acceptance, with an immutable authenticated absolute session deadline. Default the choice to fifteen minutes; the host explicitly selects a longer limit. Grant refresh/reconnect cannot extend it. Existing v1 remains capped at fifteen minutes; incompatible clients must fail closed.
5. Authenticate the exact stream incarnation, publisher account/device, room/call, helper account/device, endpoint, permissions, generations and deadline. The stream can predate the Assist request; the chosen Assist duration starts at acceptance, not at the original stream start.
6. Stream stop/unpublish/capture end terminates all associated grants and pending requests even if the room or voice call continues. Enforce authenticated lifecycle plus short connectivity/revocation leases; a frozen page or lost network cannot leave two hours of native authority. Republish, source replacement, eligibility loss and reconnect cannot revive or broaden old grants. Define picker cancellation, late-result refusal and native stop reporting without unsafe Electron grants.
7. Deliver exact canonical types/schemas, state transitions, limits, errors and executable positive/negative fixtures for Windows. Choose field names in this canonical contract; the Windows proposal is not a second wire format.

The Windows offline helper currently requires bind(envelope, expectedBinding, originalSessionDeadline), still with its fifteen-minute legacy ceiling. It has per-capability AbortSignals and terminal cancellation, but no authenticated stream lifecycle or real issuer/device verifier. These are private helper APIs, not required wire names. Return the verified new contract before Windows changes its lifetime policy or advertises support.

## Implement the companion responsibilities

Add stream/group admission, request/invitation/permission/time UI, authoritative lifecycle/revocation and adapter behavior using the amended contract. Reuse existing control/data/transfer components. Keep actual control unavailable until the approved native endpoint independently confirms authorization and local consent. Guests cannot approve themselves or silently take over.

Preserve MatrixRTC/LiveKit media E2EE. Use a reviewed authenticated encrypted data path with device/session/fingerprint binding; do not enable general canPublishData, reuse media keys ad hoc, add plaintext fallback or store private transfer contents on servers. Keep release/control traffic independent of bulk-file backpressure.

Integrate existing bounded input, focus-loss release, source generations, independent clipboard directions and file acceptance/cancellation with the stream scope. Preserve browser gestures and save restrictions. Reuse the existing annotation architecture with Assist-specific grants. No remote filesystem browser, silent overwrite or automatic execution.

Publish the first verified contract/types/fixture commit as soon as it is usable for Windows integration; continue the remaining web work in coherent slices. Do not postpone the integration handoff until every UI convenience is finished. If identity or transport cannot be established safely, provide exact findings and a concrete reviewed proposal while completing independent parsers/service negatives. Never invent successful verification.

## Review, verification and handoff

Owner architecture approval is already recorded under #33; do not ask for the same approval again or claim the independent review is complete because of it. Review the Windows native design and amended stream authority from the companion boundary, record concrete findings/resolution against exact revisions, and retain the independent-review requirement before privileged input. Backend code is not permission to install or activate native execution.

Run applicable High-risk checks, real service/Matrix/LiveKit negatives and browser acceptance. Cover wrong stream/publisher/room/device, group invitation without approval, simultaneous responders, controller replacement, exactly two hours and over-limit/overflow values, old v1 long-grant refusal, stream end before/during consent and input/transfers, voice/room continuing after stream end, delayed packets, republish/reconnect replay and unchanged clipboard/file integrity/permission negatives.

Mocks prove component behavior only. Real Windows and cross-client acceptance remains Not Verified until executed with the Windows implementation. Keep unsupported production controls disabled and update practical user/admin/operator guides and the canonical Windows handoff.

Commit verified slices on main, push normally, report exact-SHA CI, and update #18/#4 and relevant assigned interventions. Return the final web SHA, protocol version, shared fixture paths/hashes, proof-verification requirements, exact Windows APIs/messages to implement, actual test evidence and remaining gates. Use Implemented / Experimental / Planned / Blocked / Not Verified accurately.

No deployment, production activation, endpoint installation or partial Assist release is authorized by this handoff.
