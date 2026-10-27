# Assist continuation checkpoint

Windows main continues from 2f250f6836d1d23bc2228ea812d575d3369550ac. Companion main was refreshed read-only at ae89e414ec71a193094ef7cbee5ad678418ff6f6. Preserve the existing checkout and updater/session identity. See ASSIST-RELEASE-PLAN.md for the ordered implementation and acceptance requirements.

## Completed component work

Native-confirmed enrollment now binds the installation key and publisher tuple, revokes its sessions on replacement, and checks global stop throughout dispatch and acknowledgement. Capture now validates the live selected ICE pair, actual peer certificates, reciprocal ports and fresh native socket ownership, retaining Chromium's exact address-redaction marker as unknown metadata.

Current executed evidence is in TEST-RESULTS.md. Previous 2f250f6 CI failed capture verification; it must not be represented as passing. Independent review cleared these disabled components, not production activation. Require fresh package verification and green CI on the pushed candidate SHA.

## Remaining Windows integration

1. Build the owned remote input peer with actual observed helper certificate checks and canonical signaling.
2. Add typed live native coordinator/IPC, concurrent cancellation and stop, enrollment/grant consent wiring, and trusted local host controls. Do not reuse diagnostic immediate-exit cleanup for active input.
3. Connect capture/source, grants and independent leases. Treat the publication SID as a tentative page claim and describe enforceable input scope accurately.
4. Add canonical native resource adapters after the companion contract and helper authority are finalized. Add quit/update cleanup barriers that retain the last cleanup holder until exit.
5. Execute approved disposable-device, cross-client and installed-upgrade acceptance before preparing a fresh release version/tag. Never reuse the stopped v0.3.0 tag.

## External dependencies and approvals

Companion source stays read-only; the current Claude prompt is ASSIST-COMPANION-HANDOFF-PROMPT.md. Windows#4 tracks resource/data-plane, relay, source-trust, cleanup and page-side safety-queue work. Production issuer setup remains project-palisade#47. Windows#5 tracks disposable-device acceptance; no test-device answer has been received.

Architecture approval and independent reviewer authorization already exist. Installation, native activation and release publication still require separate owner approval. No real input or clipboard testing on the owner's personal desktop. Production remains disabled; full Assist is not release-ready. Window-size persistence remains queued after Assist readiness.
