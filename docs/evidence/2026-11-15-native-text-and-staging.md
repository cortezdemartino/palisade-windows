# Native text, guarded consent and private file staging

Campaign: M4G-ASSIST-FINAL-6. Starting Windows commit
`40e833ca6274a18a531cd82e5c781a0c37542ffb`; campaign baseline `ca983d0`.
Companion refreshed read-only at `84b379f730d13490dad2417b30065442152f0035`.
No release, signing, installation or production activation was performed.

## Implemented integration

The existing native registry, gate, lease poller, consent, private channel and
owned DTLS gateway now carry manual plain-text clipboard operations. This is
qualification-only integration, not real Windows clipboard acceptance.

* A closed private-v3 ResourceText request uses ordinary capacity. Cancellation
  and Stop retain reserved independent capacity. No general clipboard API is
  exposed to hosted JavaScript.
* Frozen clipboard-text envelopes enforce the stream/session, directional grant,
  generation, safe-integer sequence, exact schema and bounded Unicode payload.
  Incoming replay state is consumed before consent. The original peer, grant,
  request cancellation and unchanged signed authority remain bound through UI.
* Sensitive confirmation terminally stops input and joins its retained worker
  before showing. Viewing and independent resource authority remain separate.
* The native CF_UNICODETEXT adapter reads only after Send approval; incoming text
  requires Copy approval before replacement. It validates UTF-16 and size, owns
  clipboard/window/global-memory cleanup, and exposes no other clipboard format
  or background monitor. Tests unconditionally refuse OS clipboard calls.
* Native-produced outgoing bytes require an exact SHA-256, original request/peer
  and current authority check immediately before gateway admission. The permit is
  one-shot. Cancellation while verification is waiting prevents transmission.
  An already admitted SCTP send cannot be retracted; status does not claim remote
  copying or receipt from local send completion.
* Public extension results/events contain transfer metadata only. A clipboard
  failure after an OS write may have started is described as unconfirmed, never
  falsely as an untouched clipboard. Cleanup failures are sticky and revoke the
  native runtime instead of reporting success on retry.

A canceled resource-only transport may reconnect without reauthorizing old input
or settling its cleanup mirror. First input on a resource-only transport retires
that exact transport only after verified native grant acknowledgement. The same
session registry, immutable deadline and original cleanup owners remain.

## Consent and capture safeguards

Shared Rust Allow controls now require matching fresh DOWN/UP messages with
accepted Windows input origin and device type, focused/visible/enabled current
window, ordinary desktop and current cancellation/deadline. Unknown, system and
ordinary injected sources, stale/same-tick input, repeats without a fresh press,
bare button notifications and automation commands cannot approve. Focus/capture
loss and invalid gestures disarm approval. Deny, Close and cancellation win over
reentrant cosmetic messages.

InputOwner excludes live and cleanup injectors, but its release does not drain
queued OS input. The final gesture check closes that separate approval path.
Windows includes trusted UIAccess software in its hardware-origin class; this is
not proof of a physical person against a compromised local desktop. See Microsoft's
[message-source API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getcurrentinputmessagesource)
and [origin definitions](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ne-winuser-input_message_origin_id).

Window destruction requires positive HWND/WM_NCDESTROY evidence before releasing
its referenced Context and input exclusion. Failure retains the worker and owner
as cleanup pending; no timeout or process kill manufactures successful cleanup.

The Electron picker is provisional. Native source verification and a fresh
app-owned confirmation precede active capture, borrowing the same retained input
exclusion. Source/launch cancellation and source liveness remain bound through
the decision. Native confirmation is bounded to 60 seconds, its private RPC to
65 seconds, the overall picker/approval/media workflow to 110 seconds inside the
frozen 120-second request budget. The source picker itself still has 30 seconds.
Denied/canceled confirmation retires pending native selection. No personal source
or actual native confirmation was exercised.

Window process/handle binding still lacks an independently qualified destruction
generation witness against same-process HWND reuse. This existing limitation
remains open for immutable window/source acceptance; the new prompt is not proof
that every replacement race is solved.

## File primitives and reproduced reparse defect

Receiving now creates a random private delete-on-close stage in the isolated native
store, without exposing the remote name or asking for a final destination early.
Completion rehashes the actual held file and size, not merely the incremental
network hash. Final native Save is followed by separate fresh local confirmation
before destination creation. Selected sends likewise confirm the exact path before
reading or hashing it. Paths stay native-only.

Save uses a private temporary beside the selected destination, copies from the
held verified source, rehashes the actual copied file, writes/flushed Mark of the
Web, and publishes atomically with replacement disabled. It works by copying
between locations rather than assuming staging and destination share a volume.
Actual cross-volume Save and picker behavior remain Not Verified.

Independent review identified a real gap in the older parent-directory guard:
denying rename did not prevent an empty held directory becoming a junction.
Four disposable NTFS fixtures reproduced this in-place mutation and verified the
correction. Every opened directory is bound to its held predecessor; source,
private stage and helper-image handles are checked against the actual volume-GUID
parent and basename before content I/O. Redirected creates remain empty and delete
on close. Held children make every retained ancestor nonempty, preventing later
in-place conversion. No extra pre-open attribute poll is treated as proof.
Microsoft documents the [nonempty-directory restriction](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/4aeefef8-92c3-4abc-af7a-a610caf8a165).

Save admission rechecks the original request after waiting for the store lock and
before extending its operation timeout. A deterministic regression prevents an
expired previously checked request from being revived. Signed session/grant
deadlines remain immutable.

These are file primitives, not live file-transfer dispatch. Generic peer loss still
deletes staging. Resume needs an authenticated genuine-loss transition before peer
cancellation, retained maintenance authority, and fresh grant/peer/native consent;
no indefinite or silently resumed transfer was introduced.

## Qualification packaging

The isolated builder defaults to capture-only. Explicit --native-input and
--clipboard-text selections map to a closed Cargo feature allowlist, require an
independently supplied public test profile, and build a distinct unsigned unpacked
application. Production packaging rejects these selections and verifies actual
compiled binary flags. Build metadata remains non-authorizing.

The wrapper verifies packaged helper/ASAR hashes, unchanged clean source and
temporary-profile cleanup before recording success. Existing evidence is preserved;
dirty source, a changed profile, mismatched helper features or failed cleanup
cannot create a success report. Reports explicitly distinguish compiled features
from endpoint startup, device acceptance and release approval. No approved profile
was supplied, so actual sensitive qualification packaging remains Not Verified.

## Verification and limits

Local Windows checks on this source:

| Command / evidence | Result |
| --- | --- |
| npm run check | 290 passed, none skipped |
| npm run test:assist-native | fmt, strict all-target/all-feature Clippy; 304 tests passed in each of default, all-feature and resources-only configurations |
| npm run test:assist-peer | 20 real Electron DTLS fixtures with synthetic payloads/fake input sink |
| Focused windows::files | 36 passed, including four actual NTFS junction fixtures and delete-on-close process exit |
| npm audit --omit=dev | 0 vulnerabilities |
| cargo audit --file native/assist/Cargo.lock --deny warnings | Passed |

Process checks passed: 44 tests each against default and all-feature Probe helpers, plus hidden-startup owned-control visibility. NSIS packaging, update/Assist package integrity, packaged window controls and 30 permission boundaries, isolated capture with 12 negative cases, and 20 packaged DTLS fixtures passed. No installer was run. Installer SHA-256: 8705e27dbea3f91590b2eb996f04981502a0cfa6bd1b0b01b5a9663d650588d3. Helper SHA-256: c4108b635d5101d2386205e92bd3fc251b2277a626611b979cf9656f0f11e5c6. Exact pushed-SHA CI is tracked in Windows #1.
Earlier fixture-order/Save-phase test defects and Clippy's large enum were fixed
before this successful run. Focused independent source review found and corrected
the consent notification/lifetime, timeout and reparse issues above; it is not the
integrated candidate review required under #33.

No test read or overwrote the personal clipboard, injected OS input, opened an
actual sensitive native picker or transferred a user's file. Filesystem tests
used disposable fixture directories and ordinary no-authority child processes.
No real service endpoint was enrolled. All 30 real cross-client cases and native
consent/clipboard/input acceptance remain Not Verified. LocalYield/seamless
cooperative delivery remains Blocked, and public feature switches remain unchanged.

Frozen compatibility was independently rechecked against companion main: six
source blobs, nine fixture families, all 30 cases and manifest SHA-256
`f9ac9d835333e341907ae63f2d2a497ef4944607ea60fc344e81ad64115606b0` match.
Companion af21c9f now fixes stale-loss conditional revocation and scoped input Stop;
this is source-inspected here, not a combined Windows acceptance run. The current
handoff requests late-channel reconciliation rather than repeating resolved bugs.
