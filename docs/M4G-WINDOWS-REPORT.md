# M4G-WINDOWS preparation report

2026-10-24. **Overall classification: Blocked.** The required web contract is absent. Implemented changes are an unpublished permission-hardening candidate, version 0.2.12, under the assignment fallback. They are not the requested complete view-only bridge.

## Revision evidence

1. Starting Windows SHA: a72055f8270eca79a17d0b64b3b85c768fe65748.
2. Final Windows SHA: the main commit containing this report; the exact SHA and Actions run/result are recorded in [Windows #1](https://github.com/cortezdemartino/palisade-windows/issues/1) after push and in the final handoff. At report creation CI is pending, not passed.
3. Compatible web SHA: **none established**. Audited web main 231e96b83230834c7419779c311236b3330dc3bc contains no desktop contract; its source was read-only. Local companion SHA 7da19ff0a5db0fd814ce8f73417d303b0d89759d was not used as a substitute for main.
4. Bridge protocol: **Blocked / undefined**. Existing backend Assist v1 is not a desktop IPC version.

## Implementation and acceptance

5. Changed files: src/main.cjs; new src/session-policy.cjs; tests/desktop-window.test.cjs; new tests/session-policy.test.cjs; new scripts/session-policy-smoke.cjs; scripts/test-window-controls.cjs; .github/workflows/windows-build.yml; package.json/package-lock.json; README.md; RELEASE-NOTES.md; docs/SECURITY.md; docs/USER-GUIDE.md; docs/OPERATIONS.md; docs/DEVICE-SETTINGS.md; docs/TEST-RESULTS.md; new docs/ASSIST-INTEGRATION.md and this report.
6. Preload/IPC design: **Blocked** for Assist; no speculative privileged interface. Existing isolated unread observer/local controls remain unchanged. **Implemented** shared session policy extracted from main, not duplicated.
7. Capability fields: **Blocked**; no desktop schema exists to validate. No native endpoint, input or emergency-stop availability is asserted.
8. Display capture: **Implemented refusal only**. Explicit handler returns null; no system picker, enumeration, metadata or grant. Existing capture deny policy retained.
9. Source picker: **Blocked**, not implemented; no first-screen default.
10. Capture lifecycle: **Blocked** pending web generation/revocation/termination contract. Denial creates no native capture state. Current contents replacement, navigation and destruction invalidate permission checks.
11. Host consent: no capture can start in this candidate. Future local picker consent and independent stop authority are required; no website/session identifier authorizes input.
12. Backend authorization integration: **Not Verified**; no backend changes or new authorization path. Existing browser media/service implementation reviewed at the audit SHA.
13. E2EE: **Not Verified** in Windows cross-client operation. MatrixRTC/Rust/LiveKit ownership preserved; no secrets or plaintext streaming added.
14. Web to Windows: **Not Verified**.
15. Windows to web: **Blocked / Not Verified**.
16. Windows to Windows: **Blocked / Not Verified**.
17. Browser regression: **Not Verified** in this assignment; no web source changed.
18. Native Windows: real Electron 44.7.2 fixture ran on Windows x64 with synthetic devices. Physical installed-client capture, multiple monitors, high DPI, lock/sign-out/sleep, network interruption and real-session termination: **Not Verified**.
19. Security negatives: **Implemented and passed** unit checks for wrong/null/spoofed contents, same-origin subframes, missing main-frame provenance, conflicting origins, wrong current origin, stale/destroyed contents, unrelated device permissions and display refusal. Existing local IPC spoofing and update tampering checks retained. No Assist IPC version/rate/size negatives can run against an absent protocol.
20. Unit tests: **Passed**, 49 tests via npm run check. This includes existing update signature/key rotation/installer mismatch and profile/settings requirements. npm ci passed; production audit reported zero vulnerabilities.
21. Electron integration: **Passed** source and packaged controls/unread/permission fixtures. Packaged synthetic microphone allowed/stopped; notifications permission allowed without a toast; camera/mixed media/display/same-origin subframe/wrong-contents microphone and external navigation denied. No privileged page globals. Synthetic fixtures are not physical acceptance.
22. Packaging: **Passed**, Windows x64 NSIS 0.2.12. Packaged source policy loaded from app.asar. Candidate not installed. Authenticode not applied.
23. Signing/update regression: **Passed** existing automated signer/verifier/key rotation/hash/installer-helper tests. Production trust key, fuses, app ID, persistent partition, manager pin and NSIS data preservation unchanged. No new production manifests signed; no authenticated installed upgrade executed.
24. Performance: **Not Verified**; no screen capture startup/picker/latency/CPU/memory/frame-rate claim.
25. Documentation: **Implemented** authoritative contract audit, actual boundaries, missing definitions, matrix, browser-only user workflow, acceptance/recovery gates and updated relevant Windows guides.
26. Enhancement tracking: [Windows #1](https://github.com/cortezdemartino/palisade-windows/issues/1) and [parent #18 update](https://github.com/cortezdemartino/project-palisade/issues/18#issuecomment-6389869031).
27. Owner intervention: [Windows #2](https://github.com/cortezdemartino/palisade-windows/issues/2), assigned to cortezdemartino with steps, missing exports/tests and verification. Later physical accounts/devices and release decisions remain explicit gates.
28. Exact-SHA CI: **Pending at report creation**. The checkpoint push must trigger Windows build, including new packaged security checks; final result belongs with the exact SHA in Windows #1. Older green runs do not qualify this candidate.
29. Release/deployment: **Not performed**. No tag, GitHub Release, installed endpoint, production activation or web deployment. Main CI test artifacts are not approved releases.
30. Known limitations: full M4G-WINDOWS is blocked by absent committed web protocol/adapter/provider/lifecycle/tests. No real cross-client experience, picker, native stop integration or E2EE acceptance is claimed.
31. Native-control prerequisites: separate architecture/identity/scoped consent/replay/input-cleanup/emergency-stop review in project-palisade#33. No keyboard/mouse injection, service persistence, clipboard/files, elevation or arbitrary execution implemented.
32. Recommended next phase: deliver and review the committed M4G-WEB exports and contract tests, pin its compatible SHA, then implement the contracted view-only bridge/picker/revocation and run all four cross-client E2EE and physical Windows acceptance paths before requesting release approval.

Full design, source references, test commands and troubleshooting: [ASSIST-INTEGRATION.md](ASSIST-INTEGRATION.md).
