# Monitor-owned executor custody

Starting Windows revision: `a2243699da4f6f80aa6e44e20d72206e40e0bd5a`.
Its exact-SHA [CI 42718767049](https://github.com/cortezdemartino/palisade-windows/actions/runs/42718767049)
passed. Companion remains frozen at `01ae58565dd29dce692bace6d09834c5efdbff69`.
No canonical source, fixture, wire contract or production switch changed.

## Implementation

- `custody.rs`: bounded cleanup/process evidence state machine, immutable tickets,
  retained generation tombstones, pre-write Armed admission and terminal sealing.
  It neither verifies grants nor duplicates the executor's held-input ledger.
- `windows/process_tree.rs`: tracked launch errors return an actual created child
  instead of dropping its owner; strict job/handle-list inheritance is preserved.
  This API is used by the new custody path. Legacy callers are not thereby qualified.
- `windows/image.rs`: exposes internally the digest already verified from the
  locked executable, rather than accepting a caller-selected image identity.
- `windows/custody_probe.rs`: retained process/pipe owner and fixed no-input actor
  modes. Monitor-only ARM writer and child-only receipt writer are separate from
  ordinary I/O. Bootstrap/query errors signal Stop and close input/ARM while
  preserving exact process ownership. Full ordinary output is observed through
  kernel buffer occupancy while the original child remains alive.
- `tests/assist-custody-process.native.cjs`: real compiled-helper scenarios, including
  exact original handle retention and monitor death. Both native feature builds
  run the new file through the existing test command.

A valid child receipt does not settle a live coordinator mirror. Completion needs
actual child exit plus settled mirror evidence, or actual coordinator exit plus
positive child release. Zero job population and exit125 are not release receipts.
Failed, missing, truncated, duplicated or mismatched receipts cannot become success
because all processes eventually exit. Probe receipts are rejected for native mode.
Known-record completion is explicitly narrower than complete job/resource cleanup.

The private fixed actor protocol is `palisade.assist.custody-probe/v1`. It is
non-authorizing and does not modify the canonical web contract or existing private
live v1/v2 scope. No diagnostic actor can reach input hooks, injection, enrollment,
clipboard or resource APIs. The monitor's probe mirror settlement is synthetic;
normal native child/mirror wiring remains unfinished.

## Executed verification

`npm run test:assist-native` passed formatting, strict all-target/all-feature Clippy
and **168 Rust tests per configuration**: 115 library plus 53 integration. The
library total includes the two fixed subprocess fixture entry points. The new ten
custody-model groups and eleven Windows custody groups cover reservation/capacity,
slot/generation/nonce reuse, sealing, arm/receipt ordering, mode/ticket substitution,
coordinator loss, delayed exit, lost/partial ARM, bootstrap/query/post-create errors,
full ordinary output and missing/invalid terminal evidence.

`npm run test:assist-process` passed **29 groups per default/all-feature release
configuration** and the separate hidden-startup check. The three new groups execute
seven positive no-input actor scenarios, five malformed/missing receipt scenarios
plus wrong-image refusal, and forced no-authority monitor loss with original child
handles and the existing four-second exit assertion. No input was injected. Invalid
ready ticket/mode and missing-ready terminal frames are also covered by native
unit/subprocess regressions and stay failed after later plausible-looking frames.

**202 app tests**, syntax checks, production npm audit and locked RustSec audit
passed. Local x64 NSIS packaging, packaged native integrity and release panic probe
passed. Packaged Electron44.7.2 desktop/permission checks passed all **30 cases**;
isolated capture passed its **five component runs**, including **12 negative
video/lifecycle cases**; owned peer transport passed **14 fake-input-sink cases**.
These are component checks, not production service, physical input or cross-client
acceptance. Independent source review cleared the probe-only slice after the
startup-owner, sticky receipt-validation and output-pressure findings were fixed;
the reviewer ran no builds or tests.

Unsigned development installer SHA-256:
`1855473f2eaffb5892208cea094296a47152e03d01ab7c91abe31cc6f9c66a60`.
Packaged helper SHA-256:
`e8a5c8dcc9622e5adb99ed7e7eab55465192df92b6e6d2375f7396521970a516`.
Neither was installed, release-signed or published.

Reproduce from the repository root with pinned Node22 and Rust1.98.1:

```powershell
npm run test:assist-native
npm run build:assist
npm run test:assist-process
npm run check
npm run package:win
npm run verify:assist-package
```

These commands create local development artifacts, not a release. Do not install
them on a personal device or overwrite the published v0.3.3 assets.

## Classification and remaining integration

**Implemented / Verified in simulation** for the custody model, and **Implemented /
Verified on real Windows** only for no-authority process/pipe/handle behavior. No
physical input, native consent, enrollment, service session or received file was
exercised. The probe cannot provide a native cleanup completion token.

Next: the normal executor must use monitor-owned spawning and dedicated receipts,
fence every injector, settle actual child/mirrored owned intent and retain the
exact input-exclusion object through both owners' cleanup. Then complete a sealed
monitored-launch bootstrap, one launch signer and persistent session registry,
fresh-grant acquisition, protected consent, capture/transport/resource dispatch and
physical cooperative arbitration. Do not widen the private cleanup scope or expose
`localYield` on the basis of this probe. Issuer #47, TURN #48, disposable-device #5
and independent integrated review #33 remain separate acceptance gates. No release,
installation, production activation or deployment was performed.
