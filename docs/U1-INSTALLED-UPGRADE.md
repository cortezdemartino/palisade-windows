# Qualifying the built-in updater on Windows

Status: Not Verified. Use a disposable Windows 10 or 11 x64 VM with snapshots,
two Windows user sessions and an isolated Palisade test account. Keep the
production signing key and personal installation outside this procedure.

1. Record the exact v0.3.4 candidate commit and snapshot the VM. Save two
   independent source exports from that commit for a baseline and a target.
   Use a dedicated test-only GitHub release repository for delivery.
2. In both exports, set the test owner and repository in
   `src/updates/release.cjs` and the matching exact installer URL in
   `native/update/src/lib.rs`. Keep HTTPS, the asset restrictions and the
   signature validators. Give both exports the same fresh test Ed25519 public
   root in `assets/update-public-key.pem` and an empty transition list. Store
   the test private key outside both exports. Preserve the normal app ID,
   product name and NSIS GUID to exercise real installation registration.
3. Keep the baseline at 0.3.4. In the target export set 0.3.5 with
   `npm version 0.3.5 --no-git-tag-version --ignore-scripts`. Add the matching
   `release-notes/0.3.5.json` using the checked-in template with test-only text.
   Build both x64 installers with `npm ci --no-audit --no-fund` and
   `npm run package:win`. Use the project's normal pinned Manager dependency.
4. Sign the target assets with the test key using
   [the signing procedure](UPDATE-SIGNING.md). Verify the full asset set with
   `node scripts/verify-release.cjs out` in the target export. Publish the
   signed stable release and all required assets only to the test repository.
   Keep the exact artifact hashes and the public test fingerprint in evidence.
5. Install the baseline per user at a path with spaces. Open it, sign in to
   the isolated account and record the installed path, version and Matrix
   device ID. Verify encrypted messages and save tray and login preferences.
   Wait for discovery and background staging. Pause, resume and cancel once.
   Confirm the progress and signed notes appear and Update Manager stays closed.
6. Choose **Restart to update** and confirm. Record helper creation, exact
   parent exit, installer exit and relaunch. Verify 0.3.5 in About and check
   that the path, saved preferences, Matrix device and encrypted messages
   survived. Confirm Windows Search and existing shortcuts resolve to the
   new executable and the desktop records the successful version.
7. Restore the baseline snapshot and repeat with an all-users installation.
   Confirm UAC is requested when appropriate and that cancellation preserves
   the original installed version. Run the successful path with a user
   session in the tray and another Windows session active. Verify the
   installer refuses to close a running client and can be retried after
   normal exit from every session.
8. Repeat in separate snapshots with disconnected download, partial cache,
   altered installer, altered signature, unavailable disk, denied write,
   offline restart, lower version and a superseding release. Verify blocked
   Assist cleanup keeps the desktop open and prevents apply-helper launch.
   Use a local fixture holder for a stalled cleanup without enabling Assist.
9. Interrupt staging and the restart handshake separately. Check recovery
   and revalidation after restarting. Test an installation failure and verify
   the documented manual repair path without deleting the profile. Check
   that ambiguous or missing completion does not get recorded as success
   until the expected version actually opens.
10. Test the one-time manual upgrade from published 0.3.3 to the candidate
    with all Palisade clients closed before running setup. Run the
    [historical v0.2.0 to v0.2.1 procedure](UPGRADE-TEST.md) separately if
    that evidence is required.

Record Windows build, commit, scope, installer hashes, steps run and outcomes.
Keep these test builds and fixture keys out of production release workflows.
Close [Windows #8](https://github.com/cortezdemartino/palisade-windows/issues/8)
after a signed upgrade succeeds with the same profile and without launching
Manager. Retain the bundled recovery dependency until that result is recorded.
