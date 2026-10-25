# Native Assist guardian

Status: **Implemented, inactive component**. The application neither starts the guardian nor advertises an available endpoint. This is not a full Assist release candidate. It implements no mouse/keyboard injection, capture, clipboard, file transfer, listener, service or remote command execution.

## Process boundary

`native/assist` builds an unelevated Rust x64 executable from the pinned toolchain and Cargo.lock. The internal main-process broker verifies its size and SHA-256 against a build-generated manifest inside the integrity-protected ASAR. There are no runtime binary downloads. The normal guardian checks its actual creator PID and creation time, the fixed adjacent packaged Palisade executable path, matching user SID/nonzero session and absence of elevation. It retains the parent handle and removes pipe-handle inheritance. These checks establish a local process relationship, not Assist authority. Compromised main processes and sufficiently capable same-user malware remain outside this boundary.

The private diagnostic protocol is `palisade.assist.guardian` v1, distinct from web contracts. Frames have a four-byte little-endian length and at most 16,384 UTF-8 bytes. Empty, oversized, incomplete, duplicate/unknown-field and out-of-sequence requests terminate the process. A single bounded channel slot limits queued frames. A challenge/response handshake is required within five seconds. It correlates the instance and grants no remote authority.

Fixed commands are hello, status, verify-grant, enrollment-challenge, stop and shutdown. Probe mode refuses enrollment signing. The internal broker exposes only status, issuance diagnostics and terminal stop/close, with one outstanding request, size limits and deadlines. No generic command API is exposed to a renderer; production main/preload do not import it.

## Identity and verification

An optional enrollment identity uses Windows system randomness and current-user DPAPI at `%LOCALAPPDATA%\Palisade\Assist\endpoint-v1.dpapi`. No machine-wide DPAPI, plaintext seed persistence, updater keys or Matrix keys are used. Seed/decrypted buffers are cleared; dalek enables key zeroization. Only an exactly 32-byte enrollment challenge can be signed. There is no grant acknowledgment signer and no page/broker enrollment route yet.

Corrupt, inaccessible, conflicting or malformed identity files fail closed and are never silently replaced. File opens disallow write/delete sharing and do not follow a final reparse point. Directory reparse checks add refusal checks, but do not fully prevent ancestor-directory replacement races by same-user filesystem attackers. This is not a general handle-safe filesystem broker.

The Rust verifier uses strict Ed25519 verification over canonical control v1 sorted-key bytes. It validates exact fields, capability/endpoint consistency, account/device/transport binding, expected endpoint, deadline and generation. Legacy v1 remains capped at fifteen minutes. Shared companion signatures are tested unchanged. Caller-supplied diagnostic issuer/context inputs authenticate issuance only: every diagnostic result returns `authorized: false`. No fixture key or diagnostic key is enrolled as production trust or local consent.

## Independent stop

A separate watchdog checks parent death, backward/invalid wall clocks, missing handshake and five seconds without completed parent activity. It never waits on frame reads, stdout writes or the renderer. EOF, malformed input and stop are terminal. A separate Windows message thread registers session/power notifications. Normal mode requires a visible Stop Assist window and Ctrl+Alt+Shift+F10 registration. Registration, timer or message-loop loss prevents continued operation. Window creation and destruction stay on their owning thread.

Probe tests post messages to an owned hidden window and intentionally skip global hotkey registration. They are not physical hotkey, lock, sleep or protected-desktop tests. Notifications do not establish that startup occurred on an unlocked Default desktop. Initial and per-dispatch OS desktop validation is still required before input. No held-key crash cleanup is claimed: a future executor needs a surviving release authority/mirrored ledger and actual disposable-device tests.

## Reproducible verification

On Windows x64 with the Visual Studio C++ linker and the toolchain in `rust-toolchain.toml`:

```powershell
npm ci
npm run test:assist-native
npm run build:assist
npm run test:assist-process
npm run check
cargo install cargo-audit --locked --version 0.22.2
cargo audit --file native/assist/Cargo.lock --deny warnings
npm run package:win
npm run verify:assist-package
$env:PALISADE_WINDOW_SMOKE_ASAR = (Resolve-Path out/win-unpacked/resources/app.asar).Path
npm run test:window-controls
Remove-Item Env:PALISADE_WINDOW_SMOKE_ASAR
```

Packaging rebuilds the locked helper, collects dependency/Rust runtime license texts and writes the ASAR manifest. CI and protected release preparation require native tests, actual child-process probes, dependency audit and packaged integrity checks. Missing compiler, audit, licenses or verification fails the build. Existing updater signing/verification, profile partition, application identity, installer paths and fuses are preserved. This procedure does not install or activate native control.

## Recovery and remaining gates

Do not delete corrupt identities automatically. Future recovery must revoke the old endpoint/grants through the canonical authority and require explicit local re-enrollment. Do not substitute updater keys, Matrix tokens or another device's identity. Some account/password recovery operations can make DPAPI data unrecoverable; Matrix recovery remains separate.

Independent design review approved this non-injecting foundation and retained these blockers: a negotiated native bridge, endpoint-authenticated polling or issuer-signed challenge-bound short liveness proofs, versioned stream/group/two-hour authority, enforceable capture provenance and disposable-device qualification. Renderer heartbeats cannot establish unrevoked authority. See [the companion handoff](ASSIST-COMPANION-HANDOFF-PROMPT.md), [endpoint review](ASSIST-ENDPOINT-REVIEW.md) and [test results](TEST-RESULTS.md).

CI diagnostic qualification: GitHub Windows runners use administrative tokens with UAC disabled. The non-authorizing --probe mode permits that existing token context, while preserving same-user/session/creator/pipe checks and refusing identity signing. Normal --guardian always refuses either elevated token. A tested mode predicate covers every elevation combination. These hosted diagnostics do not qualify least-privilege/UIPI/input acceptance. Only fixed internal error codes can be written to stderr.
