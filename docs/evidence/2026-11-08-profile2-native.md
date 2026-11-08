# Native Profile 2 input checkpoint

Status: **Implemented component / Experimental integration; full Assist unfinished and inactive.**

Starting Windows main: `87daf784daa48d1ce3b64460dcf67fa27cb71a93`, safely fast-forwarded from local `3baa469798d7cc856d1a6263d6cd229613b15e2b`. Companion inspected read-only through `origin/main` at `ae3542fc0adb9d7f19b3e9a8521fb039555ee7f6`. No companion working-tree changes. Final Windows revision is the commit containing this report; exact CI evidence belongs to that revision and Windows issue #1.

## Implemented

- `input.rs`: separate strict Profile 2 decoder for six input kinds, sender-monotonic time, repeat, bounded NFC Unicode text, full added physical-key vocabulary and back/forward buttons. Legacy decoding retains its original allowlists. Duplicate/extra fields, malformed surrogates, controls, non-NFC text, invalid repeat and prohibited keys fail closed.
- `stream.rs`: the authoritative session gate owns the input-age guard for its single verified peer association. Authority/session/sequence checks precede age state. A valid stale event consumes replay state. Repeating certificate verification never resets age. Replacement peers and fresh grants still require the subsequent lifecycle work; this API cannot make an old peer new.
- Shared `permit_packet` is called by the actual Rust runtime worker. Trusted setup fixes the profile; packets cannot change it. Profile 2 preserves legacy input compatibility. Native queue residence stays bounded to 100 ms through dispatch, without granting a fresh window to delayed non-release work. Authorized owned releases can still clear holds. A stale discarded event returns handled success to the transport; malformed, replay and authority errors remain refusals.
- `executor.rs` and Windows input adapters expand text into individually ledgered UTF-16 DOWN/UP steps using `KEYEVENTF_UNICODE`, with permission, source, target, local modifier, stop and deadline checks. Both parent and child keep write-ahead cleanup state. The wire cannot directly mint a Unicode-unit permit. Repeat requires an already owned physical key. Back/forward use X-button data and local-held checks.
- Protected combinations include every ordering and modifier side of Alt+Tab, Alt+Esc, Ctrl+Alt+Delete/NumpadDecimal and Ctrl+Shift+Esc. Windows/OS keys remain unavailable. No protected desktop, elevation or cross-user bypass.
- Exact `unicode-normalization` 0.1.25 supplies NFC checking consistently across tests and native code; its dependency licenses are collected by the existing package pipeline. No normalization silently alters received text.
- Corrected the inherited Profile 2 provenance test's doubled local path and basename mismatch that skipped SHA-256 assertions. Both fixture hashes and all Git blobs are checked; pinned fixtures/provenance are unchanged.

## Verification

- `npm run check` under checksum-verified portable Node 22.23.4: **178 passed**, syntax checks passed.
- `npm run test:assist-native`: format, strict all-target/all-feature Clippy, **95 Rust tests in each configuration** (64 library, 15 executor, four input-model, five Profile 2, seven authority).
- Tests exercise unchanged canonical fixtures, protected chord permutations, NFC/UTF-16 boundaries, delayed owned releases, forged baseline poisoning, stale/replayed input, legacy compatibility, internal-unit refusal and interrupted Unicode DOWN cleanup ordering. Executors use fake sinks. Passing these does not establish OS input behavior.
- Initial `npm run check` with system Node 26.13.0 failed five contract test files because its TypeScript API rejects transform mode. With CI's Node 22, one inherited provenance-path test still failed; after the path/hash fixes all 178 passed. These earlier attempts are failures, not qualified evidence.
- Locked Cargo and production npm audits: zero vulnerabilities. Eighteen actual no-input subprocess groups passed in each default-release/all-features-debug configuration, plus hidden-startup visibility. Independent review cleared the inactive source and independently passed 24 Node 22 contract tests. No reviewer native input was performed.

## Remaining implementation and acceptance

The Electron private live channel still starts no authoritative session runtime. Crash-surviving descendant cleanup, guarded native approval UI, owned source/signing/peer integration, fresh input generations, native extension v2 resources, image/sync/resume/drop UI and real cross-client operation remain open Windows work. No new operational capabilities are advertised, and packaged helpers retain disabled sensitive features.

Text's existing 100 ms execution permit is unchanged. Large strings may be safely interrupted; maximum-length latency and layout/IME/AltGr behavior need approved-device measurement. Mapping Lang/IME codes does not prove layout-specific behavior. No actual OS input, personal clipboard, installer execution, production signing or deployment occurred.

Companion defect handed to [Windows #4](https://github.com/cortezdemartino/palisade-windows/issues/4#issuecomment-6724505840): mouse-only focus loss emits release-all using a keyboard-only grant mapping. Claude must reconcile canonical authority and add mouse-only held-button release tests. Windows does not silently broaden that permission. Native teardown must release owned input independently.

Physical/disposable-device and cross-client acceptance remain **Not Verified** under Windows #5; production issuer #47 and working TURN #48 remain external gates. Firefox, performance, installed upgrade and production enablement are also Not Verified. Architecture approval already exists; installation, real input and publication require their separate approvals. Do not move or reuse stopped tag `v0.3.0`.

## Build and regression attempts

The initial standalone helper build overlapped the package build while both wrote shared artifact/manifest outputs; the standalone integrity gate failed. Package generation completed successfully and subsequent verification of the packaged helper passed. This failed attempt is retained; concurrent artifact writers are not valid build evidence. The unsigned, uninstalled local installer SHA-256 is `039b6415dba2746d224bf8f4c5d0880b1dc3b11871cffce98c0128ffcc68ee47`; built and packaged helper SHA-256 is `1c25a6d54c4c9264a81b19d736091f332d4e96aa47ee72a1fa2318ef07434c5b`. CI artifacts are independent builds and need not share these local hashes.

The first packaged desktop suite timed out during its existing source-picker fixture. Added phase-specific timeout diagnostics; no assertion, permission or timeout was weakened. An isolated picker execution then passed. Root cause is not proven and the historical failure remains; a passing rerun alone is not a demonstrated fix. Broader packaged rerun and exact-SHA CI outcomes follow in the issue evidence.

Final local packaged checks passed: `npm run package:win`, `npm run verify:assist-package` (actual packaged synthetic cleanup probe), the complete `test:window-controls` rerun (including 30 admission cases), all five `test:assist-capture` groups with 12 negative/lifecycle cases, and all 14 `test:assist-peer` cases. Capture is local owned-window/synthetic media; remote input uses a fake sink. These are real component/process/DTLS tests, not physical control or end-to-end Palisade acceptance. The original picker timeout remains unresolved historical evidence. Changed Markdown relative links and syntax/whitespace checks passed.
