# Independent grant admission

Starting Windows revision: `259a4a31b9ae3368e1837f6a58bce2e935a71bf3`.
Companion remains frozen at `01ae58565dd29dce692bace6d09834c5efdbff69`, inspected
read-only. No protocol, canonical fixture or capability was changed.

## Defects and correction

The gate used its highest observed grant generation as the minimum for every
subsequent new capability. A current view can legitimately contain an older
independent resource grant and newer control grant. Admission therefore depended
on their iteration order. The constructor's trusted minimum is now immutable and
separate from the monotonic reporting high-water. Signatures, exact binding,
immutable original approval/deadline, consent and covering signed lease remain
required. Same-capability replacement is still refused.

A sticky input-admission latch prevents a previously unseen input capability from
bypassing terminal Stop. Known input revocation, input suspension (including an
empty/resource-only gate), session Stop and input expiry close new input admission.
The latch has no reset and is not cleared by an executor cleanup receipt. Resource
admission remains independent. Higher generation alone does not establish fresh
acquisition, cleanup, consent or a new authenticated peer.

Enrollment binding also incorrectly required a non-null endpoint for every grant.
Canonical resource grants deliberately have `endpoint:null`; only native input has
an endpoint field. The duplicate blanket check was removed. The existing canonical
verifier still enforces that distinction and exact participant/device/session/
room/stream/transport binding. Own endpoint/account/device/key expectations still
come from locally confirmed enrollment. Constructing a binding grants no authority.

## Verification

Formatting/strict Clippy and **147 Rust tests per default/all-feature configuration**
passed (94 library and 53 integration). Four added groups cover both mixed-generation
orders with an actual signed covering lease, constructor-floor enforcement,
same-capability/refreshed-ID refusal, terminal refusal of unseen input and canonical
resource-only enrollment binding with identity/device/signature/input-endpoint
negatives. Resource authorization still needs consent/peer/lease and survives input
revocation in the regression.

**202 app tests**, **26 no-input process groups per configuration**, hidden-startup
visibility, local x64 NSIS packaging and packaged native integrity/cleanup probe
passed. Independent source review cleared the dormant admission/binding boundary;
it ran no builds, service calls or physical tests. Full packaged desktop/capture/
peer checks passed at the earlier direct-witness checkpoint and run again in the
new exact-SHA CI.

Unsigned development installer SHA-256:
`f4989ea0bc74457e8e2d737d10ee92255037d95056af281daaf5c1c185161841`.
Packaged helper SHA-256:
`52ff9a0508780e6a63545e27b314a472cfb094f0eefbe93d448923c7fcac27d1`.
These artifacts were not installed, signed or published.

Classification: **Implemented / Verified in simulation** for initial admission and
binding. Fresh input reacquisition remains Planned: it requires bounded pending
cohorts, positive retained cleanup, new local consent and a new owned peer without
resetting old terminal flags or ending unrelated resources. The persistent launch
owner, full live runtime, protected UI, cooperative physical arbitration, extension
resources and real-device acceptance remain unfinished. No production switch changed.
