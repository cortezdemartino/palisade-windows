# Isolated Assist qualification setup

Status: preparation guide, **not device approval or acceptance evidence**.
Use alongside companion docs/52–54 and the [master inventory](ASSIST-COMPLETION-MATRIX.md).
Do not point test builds at production, use personal account data or install a
candidate without explicit approval naming the disposable device and candidate.

## Prerequisites and ownership

Existing [Windows #5](https://github.com/cortezdemartino/palisade-windows/issues/5)
is the assigned owner intervention. Supply privately a disposable Windows 11 x64
device, separate publisher/helper test accounts, and explicit candidate installation
approval. Add Windows 10 coverage where supported, multiple monitors/mixed DPI,
portrait/negative coordinates, two keyboards/mice and an international layout for
the corresponding cases. A VM covers process/service tests but does not qualify
physical host/helper arbitration.

The companion operator owns disposable Matrix, backend and LiveKit services, a
test workspace/voice room, issuer/enrollment service and a separate TURN relay.
Use companion docs/52's fixture setup and docs/54's native-mode operator workflow.
Record the exact web SHA and frozen manifest, not a mutable main branch label.
Use a dedicated test Assist issuer, never the updater signer, Matrix signing key
or production issuer. Exchange its public fingerprint over a separately trusted
channel. Keep all private keys, accounts/passwords and relay secrets out of the
repository, issues, screenshots and result records.

The November 15 host check found Docker's executable but no running Linux engine
(the local Docker API pipe was absent). No container stack was started, no approved
device/service/issuer profile was supplied, and no real endpoint was enrolled.
This is an environment observation, not proof that a remote fixture cannot exist.
Coordinate actual fixture availability through #5 instead of fabricating authority.

## Candidate preparation

1. Pin the clean Windows/web commits. Run the Windows checks, package integrity
   checks and exact-SHA CI. Retain the helper/installer SHA-256 and build version.
2. Inspect `scripts/build-assist-qualification.cjs` and the strict public
   `validateQualification` schema in `src/assist/qualification.cjs`. The existing
   builder creates a distinct unpacked application with an independently supplied
   test origin, issuer fingerprint and namespace. It does not install or launch it.
   An existing concurrent profile is refused, not overwritten.
3. Preserve the distinct qualification app identity/profile. Never substitute the
   production account/session store. A profile is public trust material, not a
   source of private signing keys or permission to act on a device.
4. Check compiled helper `--build-info` and its protected package manifest.
   The normal builder deliberately compiles native input/resources out. Do not
   assume a successful ordinary build can execute the full acceptance inventory,
   and do not bypass this check by swapping a different helper into a package.
   Sensitive qualification packaging remains a separate verified engineering step
   after the corresponding live feature and cleanup paths are complete.
5. Obtain explicit disposable-device installation approval and verify the approved
   artifact hash before an operator installs it. Existing source authorization
   does not itself approve installation or personal-desktop input testing.

## Execute and classify

Use docs/54's existing native mode and operator prompts. It signs in only as the
ordinary helper; the operator performs native choices on the approved Windows
device. Start with actual enrollment and issuer consent, then selected capture
and encrypted viewing. Do not fabricate a screen stream or native consent to
skip a missing prerequisite. Unsupported features remain Not Verified/Blocked.

Record exact Windows/web commits, build version, helper/package hashes, public
endpoint fingerprint, device label, OS/DPI/layout, manifest hash and public test
issuer identity. Keep original failed and contradictory runs. Use the strict
companion evidence parsers/evaluator to generate readiness; never hand-edit a
generated file or turn a mock report into a native report.

The frozen manifest v3 retains all 30 cases and hash
`f9ac9d835333e341907ae63f2d2a497ef4944607ea60fc344e81ad64115606b0`.
New seamless-control scenarios must be added under a negotiated successor,
without replacing old cases. Current seamless physical safety remains Blocked.

Test relay-only success against the authenticated endpoint-ice configuration,
then relay refusal/unavailability, expiry, wrong certificate and network loss.
Do not disable TLS or fall back to direct when relay-only is required. Test
Firefox on a suitable supported host with its normal security protections.

Actual input, clipboard, file, lock/sleep/crash and installed-upgrade cases require
their matching native device records. Unit/fake-sink DTLS/process tests do not
fulfill those records. Stop immediately on unsafe input or unproven cleanup;
retain the native cleanup owner and do not kill it to force a passing shutdown.

## Review and release

After implemented features pass the exact-candidate native/browser matrix, obtain
the integrated independent review under #33. Production issuer/pin (#47), required
TURN (#48), Firefox (#49) and owner approval naming exact commits remain separate
gates. A reduced preview needs its own explicit scope and approval; it is not
full-product completion. This guide authorizes no rollout or production activation.
