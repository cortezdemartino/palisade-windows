# Authenticated native relay configuration

FINAL-6 Subphase 2/3 continuation, starting Windows `62230cf13107db79e18746555b459796c72e48ac`.
That transport-only checkpoint passed [exact-SHA Windows CI](https://github.com/cortezdemartino/palisade-windows/actions/runs/43763618705).
Companion inspected read-only at `74e8fa8525677f7f0cba8d3106f00a7e65f3c042`.

## Connected implementation

The operational endpoint signer now implements the existing endpoint-ice purpose
statement and fixed authenticated HTTPS route from companion docs/50. It reuses
the launch-wide serialized request clock; there is no generic signing or URL API.
Windows retains normal certificate/revocation validation, forbids redirects,
cookies and automatic credentials, and limits allowed route/method combinations.
Cancellation is checked before and after bounded network work.

Private v3 PeerIce uses ordinary request capacity and an ingress-bound cancellation
scope. It does not occupy the live peer lane, urgent Stop reader or gate/peer/safety
locks while waiting for HTTPS. The endpoint requires the owned document, confirmed
source and exact prepared peer attempt before fetching configuration. After waiting
it checks the original attempt, source, current authority, cancellation and original
15-second setup deadline again. Old attempts cannot fetch configuration for a
successor, and canceled/unqualified Probe requests refuse before signer/network use.

The strict response decoder accepts only the actual service's bounded configuration:
up to eight unique STUN/TURN URLs, valid host/port/transport syntax, matching
ephemeral TURN username/expiry and canonical HMAC credential encoding, and all/relay
policy. It rejects missing/extra/duplicate fields, explicit null credential fields,
URL credentials/paths, invalid schemes, mixed STUN/TURN credential records, oversized
responses, expired responses and relay-only configurations without a relay.
Wall-clock rollback and a monotonic lifetime cap cannot revive expired settings.

Only trusted main receives the validated RTC configuration and passes it into the
existing app-owned isolated gateway. The hosted page receives neither relay
credentials nor an ICE-server configuration method. Native configuration, request
errors and TURN credentials are not logged or included in troubleshooting output.
The configuration type deliberately has no Debug implementation. The raw HTTPS
response is zeroized after parsing; WebRTC necessarily retains active credentials
in memory for its connection.

A lookup error fails peer setup. An expired or refused relay is never replaced
with empty direct ICE. A valid explicit empty/all response from the service still
allows direct encrypted connections. Existing actual-certificate verification,
signed transport statements, source authority and native input/resource checks
remain required with either routing policy. No frozen wire or fixtures changed.

## Evidence classification

Nine focused native tests cover the exact purpose/signature, signer substitution,
canceled no-network request, fixed HTTPS route/method policy, valid direct/relay
configuration and malformed/expired/oversized substitutions. Focused Node tests
cover private response correlation, defensive RTC validation and setup failure or
source loss without fallback. Independent source review found no critical/high
blocking finding; its extra post-network setup-deadline check was applied.

This is source/unit/private-process/transport integration. No real endpoint ICE
request, TURN allocation, restrictive-network session, production issuer or
physical input/resource action was executed. WIN-T02 and project-palisade#48 remain
**Not Verified**. An empty/direct fixture is not relay acceptance. Final candidate
test, package and CI evidence are recorded in the completion report and issue #1.

## Remaining work

Provide an approved isolated service and relay through the existing interventions,
then run direct, relay-only success, relay-only failure, credential expiry and
certificate-substitution cases using a real approved endpoint. Preserve failures;
do not alter relay policy or TLS checks to obtain a passing result.

Resource message dispatch, protected native clipboard/image/file operations,
drop/resume/sync and transfer status lifetimes remain unfinished Windows work.
Public capability flags and production activation remain unchanged.
