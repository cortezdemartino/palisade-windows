# Build and release operations

## Requirements

- Windows x64 build runner (GitHub Actions uses `windows-latest`)
- Node.js 22.13 or later and npm
- A private Ed25519 update signing key matching `assets/update-public-key.pem`
- GitHub repository write permissions to create version tags and releases

The signing key is used solely for release metadata. Windows publisher signing is optional and separate.

## Local build

```powershell
npm ci --no-audit --no-fund
npm run check
npm audit --omit=dev --audit-level=high
npm run package:win
```

Candidate output: `out/Palisade-Setup-0.3.0-x64.exe`. The automated Windows build workflow uploads an installer artifact for testing on each push to `main`. Such CI artifacts are not official releases.

## Set up release signing

The repository contains the update *public* key. Keep the matching private key out of Git history, issue attachments, chat transcripts and release assets.

Create the `release` environment in repository Settings > Environments, configure required reviewers and restrict deployment tags to `v*`. The workflow refuses publishing without required reviewers. Store the base64 representation of the private PEM as an environment Actions secret `PALISADE_UPDATE_SIGNING_KEY_B64`. From PowerShell with the GitHub CLI installed and authenticated:

```powershell
$path = 'C:\secure\palisade-update-private-key.pem'
[Convert]::ToBase64String([IO.File]::ReadAllBytes($path)) |
  gh secret set PALISADE_UPDATE_SIGNING_KEY_B64 --env release --repo cortezdemartino/palisade-windows
```

The signing script confirms the private key matches the public key shipped with the client. Release signing fails if the secret is absent or incorrect. Generate a new production key before the initial distribution if the existing private key has been exposed.

## Publish

1. Review and test the Windows build. Confirm the `package-lock.json` changes and version bump.
2. Set the version in `package.json` and `package-lock.json` to the intended release version.
3. Merge the approved changes to `main`.
4. Tag the exact reviewed commit and push the tag:

```powershell
git tag v0.2.2
git push origin v0.2.2
```

The [release workflow](../.github/workflows/release.yml) installs locked dependencies, runs the tests and production-dependency audit, builds the unsigned NSIS installer, signs all three manifests with the protected Actions secret, verifies the manifests and installer, then transfers the artifacts to a separate publish job. Build and signing use read permissions; only the publish job receives `contents: write`. That job verifies the artifacts again before creating a public GitHub Release. It rejects tags that do not match `package.json`.

Required release assets:

- `Palisade-Setup-<version>-x64.exe`
- `release.json`: v1 signed SHA-512 manifest for clients using this format and the current trust anchor
- `release-v2.json`: v2 signed SHA-256 and SHA-512 manifest for clients using this format and the current trust anchor
- `release-v3.json`: signed download URL and authenticated transition chain for v0.2.2 and later
- `SHA256SUMS`: conventional installer checksum

Do not upload unsigned or unverified replacement assets under an existing version. Publish a new tag and version for each release. GitHub Releases must remain publicly downloadable; a private repository requires another authenticated distribution design.

## Compatibility and response

The 0.1.0 release has no updater. The sole existing test installation requires the explicitly authorized manual v0.2.2 bootstrap to establish the new signing key. The application ID `org.onyxops.palisade` and the `persist:palisade-desktop` browser partition are unchanged, preserving the usual desktop profile on upgrade. Verify this in Windows testing.

If a release is withdrawn, remove it from the latest-release feed through GitHub release management and publish a fixed version. Follow [UPDATE-SIGNING.md](UPDATE-SIGNING.md) for authenticated future rotation and separately authenticated recovery after key loss or compromise.

No OnyxOps App Store, Palisade web server or Nginx changes are required for this distribution method.

Validate artifacts locally with `node scripts/verify-release.cjs out`. This requires no private key. Never replace the pinned public key to make an unknown private key pass. Remove the repository-scoped signing secret after the environment-scoped secret is configured and tested.

The reproducible upgrade procedure is in [UPGRADE-TEST.md](UPGRADE-TEST.md). The current execution evidence and release prerequisites are in [TEST-RESULTS.md](TEST-RESULTS.md).

The sole earlier test client requires the explicitly authorized manual v0.2.2 bootstrap because its original private key was lost. The old tagged public key does not authenticate the new key. See the signing guide for fingerprints, operator scope and the production publication gates. After future rotation, retain the bootstrap private key as the protected legacy-signing secret while compatibility clients exist.

## M4G-WINDOWS preparation gate

The current candidate has a compatible v1 bridge but is blocked by the Electron capture permission review, native approval and expanded data-contract dependencies documented in [ASSIST-INTEGRATION.md](ASSIST-INTEGRATION.md). Main builds include packaged permission-boundary checks. Do not tag, publish, install or activate this candidate without the required owner approval. CI artifacts are testing evidence, not approved releases. Physical capture and cross-client E2EE acceptance remain unexecuted.
