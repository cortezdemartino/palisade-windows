# Stream-bound group assistance

Owner-approved product direction, 2026-10-25. Architecture approval is recorded in [project-palisade#33](https://github.com/cortezdemartino/project-palisade/issues/33#issuecomment-6399951819). Status: Planned; group requests, two-hour grants and authoritative stream-end integration are not implemented in Windows.

## Request flows

- An authenticated, eligible viewer can request to assist the streamer, including while watching a group stream.
- The streamer can ask the current group for help or invite one specific eligible user.
- Requests can propose permissions and a duration. The streamer makes the final decision on the helper, exact device, permissions and time limit.
- A group solicitation invites responses. It never grants control to the group, the first respondent or every viewer automatically.
- Keep one native controller active at a time. Selecting a replacement requires a new explicit decision and invalidates the previous controller before the replacement can act. Simultaneous acceptances must be resolved atomically by the service.

Existing room/call/stream visibility and membership policy still applies. An invitation does not bypass access rules. Request spam needs bounded queues, deduplication, rate limits and expiry; requests cannot interrupt a stream with automatic native approval prompts.

## Permission and time scope

Viewing, visual pointers/annotations, mouse, keyboard, clipboard directions and file directions remain separate. Streamer-selected permissions are an upper bound; native local consent can refuse or narrow them. Every receiving file still requires its own acceptance/save decision.

The accepted Assist session has a positive streamer-selected duration of at most two hours from approval. Default to fifteen minutes and let the streamer explicitly choose a shorter or longer limit up to two hours. Its absolute deadline is fixed when accepted. Grants cannot exceed that deadline, native connectivity leases or the active stream lifetime. No silent renewal, reconnect or new grant extends the same session's original deadline. More time requires a new explicit session decision while an eligible stream is still active.

Every grant must be authenticated and bound to the exact stream incarnation, publisher account/device, room/call, helper account/device, endpoint, capabilities, generation and deadline. Canonical field names and encoding belong to the companion contract, not this document. Room membership, role, request text, display name or a renderer boolean is not native authority.

Window-only viewing remains view-only unless the streamer separately approves an enforceable desktop-control scope. Desktop control permits operating the user's applications; turning off clipboard synchronization does not prevent ordinary copy/paste within those applications.

## Stream end is terminal

Stopping/unpublishing the stream or ending its authorized capture terminates every Assist permission and pending request tied to that stream, even if the voice call or group room remains open and time remains. Native enforcement must invalidate authority, reject queued input, release tracked keys/buttons and cancel clipboard/transfers before network notification.

The native endpoint must receive authenticated authoritative lifecycle information and enforce a bounded connectivity/revocation lease. A two-hour maximum must not allow two hours of offline control after stream end. A frozen hosted page cannot be the only revocation path.

Losing stream eligibility, helper disconnect, host lock/sleep, endpoint uncertainty or local Stop ends affected authority. Restarting a stream with the same title or source creates a new incarnation and cannot revive old grants. If source changes broaden access, suspend/end the old scope and require a fresh explicit decision; do not silently carry authority to another monitor/window.

## Version compatibility

At inspected companion revision ee9c2763f19e62b405bbf9a9c97f187bc5a1291a, docs/47 defines palisade.assist.control v1 for pairwise Assist calls with a fifteen-minute ceiling. The owner has approved a new versioned stream/group model, not a client bypass of that existing contract. Keep its signature validation, lifetime limit and fixtures intact; negotiate an explicit compatible contract before accepting longer sessions.

The description-only palisade.assist.bridge v1 also stays unchanged. Current Windows helpers and capture controller still enforce their fifteen-minute limits and production capture remains denied. They cannot be advertised as supporting the new model until canonical parsing, stream binding, local enforcement and acceptance tests exist. The published fixture-only issuer seed must never become a production trust anchor.

## Required acceptance evidence

1. Eligible viewer requests, streamer-to-group solicitation and streamer-to-specific-user invitation, in direct and group streams.
2. Wrong participant/device/room/publisher/stream rejected; access rules never bypassed.
3. Permission escalation and guest self-approval rejected; group invitation alone grants nothing.
4. Simultaneous responders cannot acquire conflicting native control; replacement invalidates the old controller first.
5. Positive duration, exactly two hours, over-limit, malformed/overflow and immutable-deadline cases; old control v1 still refuses long grants.
6. Stream ends before acceptance, while consent is pending, with keys/buttons held and during stalled clipboard/file work; no subsequent action succeeds.
7. Voice call/room stays open after the stream ends; old Assist authority remains terminal.
8. Stream restart, delayed packets, source replacement and reconnect cannot revive or broaden an old grant.
9. Frozen renderer, network loss and native lifecycle/security transitions stop operations through independently tested enforcement.
10. Exact Windows/web revisions, shared contract fixtures, real service negatives and native/cross-client tests. Simulated stream events alone are not native acceptance.

The companion repository stays read-only in this chat. Use [the scoped companion prompt](ASSIST-COMPANION-HANDOFF-PROMPT.md) for Claude. Independent security review, native implementation, disposable-device installation, hardware acceptance and release/activation retain their applicable gates; owner architecture approval should not be requested again for the same design.
