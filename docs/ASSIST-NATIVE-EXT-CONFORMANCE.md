# Assist native resource extension: canonical Windows conformance checkpoint

Status (2026-11-07): **Pinned reference/fixtures only. Windows runtime and joint acceptance NOT VERIFIED.**

This imports the exact web `palisade.assist.native.ext` v1 reference validator and fixtures from
`cortezdemartino/project-palisade` commit `82ac7de4ec2bbd5cc2fd48f3d15dbde6ea5fd375`.
Do not change the existing `palisade.assist.native` v1, description bridge v1 or control v1/v2 wire formats.

## Files and local checks

- `tests/fixtures/assist-native-ext-v1.json`: upstream valid/invalid request, response, event and data-plane fixtures plus signed relay vector.
- `tests/fixtures/assist-native-ext-v1.web.ts`: pinned upstream reference parser. The provenance file pins both Git blobs.
- `tests/assist-native-extension-contract.test.cjs`: parses all published cases, refuses malformed envelopes, tests v2 resource grammar and verifies signed relay bytes.
- Focused command: `node --test tests/assist-native-extension-contract.test.cjs`.
- Baseline command: `npm run check`. Full high-risk Windows checks still required for sensitive native changes.

**Passing tests only prove compatibility of pinned reference inputs.** The Windows endpoint does not yet implement extension negotiation, native clipboard/file actions, local consent, resource transport or real cross-client operation.

## Integration sequence (existing issues Windows#1/#4, web#18)

1. Add a strict, separately negotiated extension dispatcher to the current authenticated native coordinator; refuse page-supplied paths, text, destinations, approvals and generic invocation.
2. Wire `resource.send-text`, `resource.offer-file` and `resource.cancel` through local Windows consent, selected handles and the existing encrypted resource peer. Never advertise a capability before it works.
3. Preserve dedicated urgent-stop capacity; revoke on lease or source loss; distinguish revocation from proven native cleanup completion. Keep Windows quit/update barriers.
4. Integrate the separately pinned docs/51 Profile 2 and extension v2 contracts for image clipboard, optional sync, verified transfer resume, safe drag/drop and fresh controller acquisition. These contracts now exist at companion `ae3542fc0adb9d7f19b3e9a8521fb039555ee7f6`; they are no longer missing web foundations. Native input decoding/execution and age admission are described in the [November 8 evidence](evidence/2026-11-08-profile2-native.md). Extension negotiation and resource execution remain unfinished.
5. Qualify with disposable Windows devices and browsers: live E2EE, both resource directions, active control, working TURN path, adversarial authorization/replay/cleanup tests, updates and performance.

Owner approval for the native design is recorded in web#33. Physical input testing, installation, activation and production publication remain separately gated. Production issuer setup is web#47; TURN provisioning is web#48; hardware and independent acceptance are Windows#5.
