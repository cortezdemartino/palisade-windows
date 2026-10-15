# Build and release operations

## Requirements

- Windows x64 build runner (GitHub Actions uses `windows-latest`)
- Node.js 22 or later and npm
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

Output: `out/Palisade-Setup-0.2.0-x64.exe`. The automated Windows build workflow uploads an installer artifact for testing on each push to `main`. Such CI artifacts are not official releases.

## Set up release signing

The repository contains the update *public* key. Keep the matching private key out of Git history, issue attachments, chat transcripts and release assets.

Store the base64 representation of the private PEM as repository Actions secret `PALISADE_UPDATE_SIGNING_KEY_B64`. From PowerShell with the GitHub CLI installed and authenticated:

```powershell
$path = 'C:\secure\palisade-update-private-key.pem'
[Convert]::ToBase64String([IO.File]::ReadAllBytes($path)) |
  gh secret set PALISADE_UPDATE_SIGNING_KEY_B64 --repo cortezdemartino/palisade-windows
```

The signing script confirms the private key matches the public key shipped with the client. Release signing fails if the secret is absent or incorrect. Generate a new production key before the initial distribution if the existing private key has been exposed.

## Publish

1. Review and test the Windows build. Confirm the `package-lock.json` changes and version bump.
2. Set the version in `package.json` and `package-lock.json` to the intended release version.
3. Merge the approved changes to `main`.
4. Tag the exact reviewed commit and push the tag:

```powershell
git tag v0.2.0
git push origin v0.2.0
```

The [release workflow](../.github/workflows/release.yml) installs locked dependencies, runs the tests and production-dependency audit, builds the unsigned NSIS installer, signs `release.json` with the Actions secret, then publishes both assets as a public GitHub Release. It rejects tags that do not match `package.json`.

Required release assets:

- `Palisade-Setup-<version>-x64.exe`
- `release.json`

Do not upload unsigned or unverified replacement assets under an existing version. Publish a new tag and version for each release. GitHub Releases must remain publicly downloadable; a private repository requires another authenticated distribution design.

## Compatibility and response

The 0.1.0 release has no updater; installing 0.2.0 once manually is required. The application ID `org.onyxops.palisade` and the `persist:palisade-desktop` browser partition are unchanged, preserving the usual desktop profile on upgrade. Verify this in Windows testing.

If a release is withdrawn, remove it from the latest-release feed through GitHub release management and publish a fixed version. Compromised signing keys require a new client build with a rotated public key, plus a separate trusted reinstallation path for clients that cannot verify a new key.

No OnyxOps App Store, Palisade web server or Nginx changes are required for this distribution method.
