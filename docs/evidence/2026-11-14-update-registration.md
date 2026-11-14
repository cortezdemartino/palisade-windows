# Built-in updater installation-name repair

Starting commit: `9968b13aa2814a168e59e50812ccc59505bd9231` (published v0.3.5).
Patch version: 0.3.6.

## Observed failure

The installed v0.3.4 client had a matching native-helper integrity record and
default-enabled automatic checks/downloads. Read-only Windows registration
inspection found an all-users installation with DisplayName **Palisade 0.3.4**,
DisplayVersion **0.3.4**, the expected publisher and the correct installation path.
No registry or installed executable was changed.

An isolated, status-only Windows process using the installed helper returned
`idle`. Starting the existing client then completed a real automatic check and
discovered signed v0.3.5. It recorded a pending download but created no native
stage/cache directory. Neither Manager nor an installer was launched. Restart
and installation were not authorized or executed.

The native `select_scope` function required DisplayName **Palisade** exactly.
NSIS writes the product name followed by its version. The mismatch rejects the
registered installation before download preparation. Manager is a separate repair
action, not an automatic redirect from `check` or `downloadUpdate`.

## Correction and regression

Accept exactly **Palisade** or **Palisade <installed version>**, retaining the
unversioned form for older records. Keep publisher, DisplayVersion, executable
file identity, installation-directory identity, App Paths corroboration and the
single matching scope mandatory. Do not use prefix or substring matching.

The regression using NSIS's exact versioned format failed before the change with
`installation-scope` and passed afterward. It covers both scopes and a machine
installation with user-only App Paths repair. Wrong name versions, suffixes,
extra spaces, unrelated products and ambiguous scope are refused. Existing
publisher/version/directory/image mismatch negatives remain passing.

## Executed verification

- Rust formatting and strict all-target/all-feature Clippy passed.
- 26 native updater tests passed in both default and all-feature configurations.
- The no-install NSIS policy probe passed, including running-client refusal,
  forged/consumed permits and retained exclusion. It does not run an installer.
- All 243 application tests and syntax checks passed.
- Version-matched release-note validation passed.

The release workflow repeats these checks, all desktop/native regressions,
packaging, package integrity and signature/hash verification before publication.
The protected release and exact-SHA build records identify the final candidate.

## Recovery and limits

Existing v0.3.4/v0.3.5 helpers cannot stage the patch that repairs this check.
Obtain v0.3.6 through the official Manager or installer once, preserving the
installation scope/path and account profile. No registry workaround or weaker
verification is required. Ordinary updates in the repaired client use the built-in
path; Manager remains an explicit repair option.

A completed installed upgrade, target-version relaunch and authenticated session
preservation remain Not Verified. The real observation above establishes startup
discovery and the old staging failure, not successful installation. The new scope
regressions use real Windows file/directory handles with controlled registration
records, not a new installed client. Assist remains disabled.
