# Companion Assist qualification handoff

Continue in `cortezdemartino/project-palisade` on main. The Windows repository is
read-only in that assignment. Start from current main, retaining candidate
`5fd1958358e2a56d965a4952c35eb6281d461b3e` and evidence
`ea9f927e90389677fd7a41a86fffc808027d1ce3` as inspected checkpoints.
Windows implementation checkpoint:
`2f8f258edc7b3f03b863e6af9984eb4def8f9ef2`. Companion readiness/evidence updates
through `3a75ffed2fb38a49578717a590a4fae1c7ae3735` retain the same frozen wire.

The cooperative contract is already frozen. Do not recreate it, change the nine
fixture families, weaken the 30 acceptance cases or advertise unsupported native
features. Windows still owns native source binding, normal executor cleanup
receipts, SessionGate/LeasePoller attachment, consent qualification and real input.
This is not a request to bypass those Windows implementation requirements.

## Requested qualification harness addition

The current native WIN-P01 test calls `publisher.share()` and waits for decoded
capture before inspecting enrollment. Windows' first connected endpoint supports
native enrollment while capture, input and resources remain unavailable. Add a
separately named, explicitly staged native enrollment check using the existing
canonical native-v1 methods and authenticated service enrollment/proof routes.
Do not count this stage as passing WIN-P01, WIN-C01 or full Assist acceptance.

Inspect `apps/web/src/features/assist/streamAssistController.ts::enrollEndpoint`,
`apps/web/e2e/assistAcceptance.spec.ts` and docs/49 and docs/54. Reuse their existing
account/device/service authorization. If a current publisher stream is required,
state that prerequisite accurately and use only the approved disposable fixture;
do not fabricate a production stream or loosen device binding to remove it.

The stage should verify the real Windows bridge/helper, the artifact's exact
revision/hash/profile, native operational public key, independent issuer pin,
local confirmation, canonical signature and the service's proven endpoint. Accept
truthful false feature declarations. Test denied/canceled enrollment, device or
endpoint substitution, restart/replacement and stale proof rejection. Never inject
OS input, use MOCK/native approval booleans, extend authority or disable TLS.

Document the required independent test issuer fingerprint, approved disposable
Windows device and test account coordination in the existing qualification guide
and interventions. Keep credentials/private keys out of issues and profiles.
Record actual outcomes separately from source/unit/mock evidence. Return the exact
commit and test commands to Windows #4. No wire changes, deployment, installation,
production activation or release publication are authorized by this handoff.
