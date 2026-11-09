# M4G-WEB-6 native release authority

Status: **Implemented component / Experimental integration. Full Assist remains Blocked and inactive.**

Starting Windows revision: `5ebca7f4f52f62e0460da47f768323d19a1c4e60` (published desktop v0.3.3). Companion inspected read-only at `5b82bc90e4997f35dd18273427c258fe54eefb95`. The Windows checkpoint is the commit containing this report; its exact-SHA CI result belongs in Windows issues #1/#4.

## Contract and implementation

- Independently verified and pinned `protocol.ts` blob `9d499137d469280c8798e72bed4ef88b56a318c3`, release fixture SHA-256 `1bc0231199e1c342fa885f3dd30bbe84e3db2b671c7b14c083067143bed73cc6`, and interop manifest SHA-256 `4ab9e541ac052fac32ca340a0585848e42cbc0388fde0530c493319d1ce17e03`. Existing v1 sources, seven earlier fixtures and extension source are unchanged. Tests check all eight fixture families against the manifest and preserve its 23 acceptance cases without marking them executed.
- `SessionGate` authorizes v2 `release-all` using the carried active mouse or keyboard grant. It never substitutes a keyboard grant for a revoked mouse grant or accepts a resource grant. Peer, session, consent, signed grant/generation, covering lease, source generation, sequence and execution deadline checks remain mandatory. Revoked carried grants report `revoked`.
- Valid releases bypass the ordinary input token budget; replay checks remain. Existing bounded transport/queue admission is unchanged and still needs complete live integration. This component change does not claim that congested end-to-end transport is qualified.
- The executor derives ownership solely from its unforgeable native permit's gate instance and grant ID. `release-all`, ordinary UP and repeat apply only to that owner. A second owner cannot adopt an existing physical hold through a repeated DOWN. Global modifier, protected-chord, text and shortcut checks remain on the combined ledger.
- Ownership is written before DOWN alongside the existing write-ahead cleanup mirror and removed after successful UP. Planning clones the ledger. Failed scoped release triggers independent fencing and global owned-input cleanup; terminal teardown does not depend on a remote release packet or surviving grant.
- No page API, runtime activation, production trust anchor, update behavior, session data, native feature flag or companion source was changed. Fixture signing seeds are test-only and never production trust.

## Verification

- Node 22.23.4 application/syntax checks: **186 passed**, zero failed. Focused Profile 2/extension contract checks: **12 passed**.
- Rust 1.98.1 formatting, strict all-target/all-feature Clippy and **100 native tests per configuration** passed (64 library, 15 executor, four input-model, five Profile 2, five release-authority and seven stream-authority).
- All shared release scenarios pass through the real native gate and executor with a fake sink. Additional tests cover a genuinely admitted resource grant, stale generation, revoked carried mouse authority alongside active keyboard authority, cross-instance identical grant strings, unknown UP, repeated DOWN/Repeat takeover, an exactly exhausted ordinary input bucket, replay, expiry and partial-UP failure with fenced cleanup of both input scopes.

- Release helper build passed. Eighteen real no-input subprocess groups passed in each default-release/all-features-debug configuration, plus the hidden-startup native-control visibility fixture. Locked Cargo and production npm audits reported zero vulnerabilities.
- Fresh Windows x64 NSIS packaging, packaged native integrity and its mandatory synthetic cleanup probe passed. Packaged desktop/permission/registration checks, all five capture groups (including 12 negative/lifecycle cases), and all 14 actual-DTLS/fake-input peer cases passed. The desktop harness's bad-IPC renderer terminations are intentional refusal fixtures.
- Independent read-only review found no blocking issue in this inactive slice and independently ran 26 Node contract tests successfully. The reviewer ran no native builds or OS-input acceptance.

Unsigned local installer SHA-256: `ea005a04f02d6e7cd2b432f59e88a8e92839a10f647f2e2ef759d3eb67a6313c`. Built and packaged default helper SHA-256: `c1235c5af37fbb6998d57aa67d85d4351067dbbc6aa842945f634168585d2c00`. This local development build retains package version0.3.3 but is not the published v0.3.3 artifact; it was not installed, signed or uploaded. The published release remains unchanged. These tests inject no OS input and do not establish joint acceptance.

## Cooperative host input requirement

The owner clarified that the host retains mouse and keyboard and both people can interact, with local priority. This is **Planned**. Current `ActivityMonitor` stops remote input for any non-endpoint event; this checkpoint deliberately does not turn that terminal stop into an implicit restart.

The companion needs a separately negotiated, nonterminal local-yield/native-readiness contract. It must fence pending remote work, prevent queued/in-flight input replay after yield, and allow only fresh input under current unchanged authorization after local activity ends. Native tracking must distinguish overlapping local and remote holds; release cleanup cannot assume a logical Windows button belongs exclusively to the helper after the host presses it too. Remain yielded if ownership/neutrality cannot be proven. This requires real disposable-device testing, not a fake sink.

Explicit Stop, revocation, protected UI/desktop, lock, source/peer/lease loss, replacement, expiry and stream end remain terminal. Cooperative yielding cannot restore them. The proposed two-second local-idle policy and exact companion work are in [the handoff prompt](../ASSIST-COMPANION-HANDOFF-PROMPT.md); they are not an implemented or negotiated wire contract.

## Remaining acceptance

WIN-P01, WIN-I05 and WIN-L02 remain **Not Verified** on a real endpoint. Fresh-grant admission, live coordinator/descendant ownership, guarded OS consent, extension v2 resources and cooperative yielding remain Windows implementation work. Production issuer #47, working TURN #48 and approved disposable-device qualification under Windows#5 remain open. The companion's reported 1019 web/280 service unit results and two 22-pass Chromium runs are external evidence, not tests executed here; Firefox remains Not Verified there. No installer execution, physical input, production activation, release signing or publication occurred for this checkpoint.
