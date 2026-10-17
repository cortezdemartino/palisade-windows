# Update signing and key recovery

## Current trust and test-client bootstrap

On October 17, 2026, the keys embedded in the existing `v0.2.0` and `v0.2.1` GitHub tags were inspected. Both contain public-key fingerprint:

`cd0ee3175e912041193af0a78064402667f13fcd6656d25b4a448e1ec93638f9`

The authenticated release listing returned no release objects. The operator confirmed that the only installed client is their own test executable and that they will replace it manually. The original private key is unavailable. No authenticated online transition from that original key is possible with the available files.

Version 0.2.2 therefore establishes a new trust anchor through that explicitly authorized manual test-client reinstall. It does not claim to authenticate a transition for the old executable. Do not distribute this bootstrap as an automatic update to any previously unknown old-key clients. If such clients are discovered, stop publication and arrange separately authenticated recovery for them.

The new `assets/update-public-key.pem` is canonical Ed25519 SPKI PEM, derived from `C:\Users\azkar\palisade-update-public.pem`. Its SHA-256 fingerprint over SPKI DER is:

`59337b8e1ba2145ff688c6674f5c09c633cc9c2b80e8ee57eab82eded3eaa781`

The matching PKCS#8 private PEM remains at `C:\Users\azkar\palisade-update-private.pem`. Only the public key is copied into the source and client. The signing scripts reject private-key paths inside the source tree, including resolved paths through symlinks. The installer packages an explicit public-file allowlist. Private keys are not release assets.

Install the locally built `out/Palisade-Setup-0.2.2-x64.exe` manually after inspecting its SHA-256 and the embedded public-key fingerprint. Keep the normal installation identity and browser profile. Do not clear application data to change update trust. The old test client cannot verify this bootstrap automatically. The local SHA256SUMS file is integrity evidence; verify its provenance independently when transferring the installer to another machine.

## Signing a release

Keep offline backups of the private key with restricted access. Do not paste its contents into logs, issues or chat, and do not put it in the checkout, output directory or installer. Run from the repository root:

```powershell
$env:PALISADE_UPDATE_PRIVATE_KEY_FILE = 'C:\Users\azkar\palisade-update-private.pem'
try {
  node scripts/sign-release.cjs out/Palisade-Setup-0.2.2-x64.exe out
  if ($LASTEXITCODE -ne 0) { throw 'Signing failed.' }
  node scripts/verify-release.cjs out
  if ($LASTEXITCODE -ne 0) { throw 'Artifact verification failed.' }
} finally {
  Remove-Item Env:PALISADE_UPDATE_PRIVATE_KEY_FILE -ErrorAction SilentlyContinue
}
```

The signed v3 payload binds the stable version, exact installer filename, SHA-256, SHA-512, size, publication timestamp and exact HTTPS GitHub release-download URL. The updater reconstructs the same allowlisted URL; a signed arbitrary URL is still rejected. Metadata uses domain-separated canonical bytes and an Ed25519 signature. There is no unsigned fallback. Releases at or below the running version are never installed.

Publish exactly these five assets under the matching version tag:

- `Palisade-Setup-<version>-x64.exe`
- `release.json`: original envelope and SHA-512 format
- `release-v2.json`: SHA-256 format
- `release-v3.json`: signed URL and authenticated transition chain
- `SHA256SUMS`

The older formats support clients using the current bootstrap trust anchor and remain signed with that anchor during future rotations. They do not make the regenerated key trusted by the earlier old-key test executable. Version 0.2.2 requires v3 and does not fall back to a v1 or v2 envelope.

Configure the protected GitHub `release` environment with required reviewers and tag restrictions before setting its `PALISADE_UPDATE_SIGNING_KEY_B64` secret to this key. Transfer it directly using a pipe to `gh secret set`, never by printing the encoded value. Do not leave an obsolete repository-scoped key in use. This change does not provision remote secrets, push a tag or publish a release. Actions decodes keys only into RUNNER_TEMP outside the checkout, deletes them in a finally block and uploads only the explicit release assets.

## Future authenticated rotation

Do this while the currently authorized private key is still available. Generating a new pair alone is insufficient.

1. Generate the successor pair outside the repository. Keep its private key offline.
2. With the current authorized key, create a signed transition to the successor public key and set a strictly increasing activation version:

```powershell
$env:PALISADE_UPDATE_PRIVATE_KEY_FILE = 'C:\secure\current-private.pem'
try {
  node scripts/authorize-update-key.cjs C:\secure\successor-public.pem 0.3.0 C:\secure\reviewed-transitions.json
  if ($LASTEXITCODE -ne 0) { throw 'Transition authorization failed.' }
} finally {
  Remove-Item Env:PALISADE_UPDATE_PRIVATE_KEY_FILE -ErrorAction SilentlyContinue
}
```

3. Review the resulting public JSON chain, append-only history, successor fingerprint and minimum version. Copy it to `assets/update-key-transitions.json`, set the package version to the activation version or later, then run tests and package the client. The original root public key stays unchanged.
4. Sign with the successor using `PALISADE_UPDATE_PRIVATE_KEY_FILE`. Supply the bootstrap root private key through `PALISADE_UPDATE_LEGACY_PRIVATE_KEY_FILE` for the v1/v2 compatibility envelopes. Actions uses `PALISADE_UPDATE_SIGNING_KEY_B64` for the active key and `PALISADE_UPDATE_LEGACY_SIGNING_KEY_B64` for the bootstrap root. Missing compatibility credentials stop release signing before metadata is written.
5. Verify all artifacts and test an upgrade from a prior current-root client before publishing. Retain compatibility manifests while those clients exist. Retiring legacy signing requires a separate reviewed migration decision; there is no switch to silently skip it.

Every transition contains the predecessor fingerprint, successor public key and minimum stable release version, authenticated by the predecessor's signature. Chains are limited to eight transitions, have increasing activation versions and reject cycles, substitutions, invalid signatures and early activation. The v3 envelope is signed by the final authorized key. A packaged chain must remain an exact public-payload prefix of every later chain, so updated clients refuse ancestor-key reactivation or rewritten history. Remote chains do not modify trust files on disk; the verified installer carries the chain into the next application version.

## Loss or compromise

If the active key is lost, stop release publication. An already delegated-away key cannot erase installed history. If the required current key is unavailable, there is no safe in-band rotation. Do not change the pinned key and label the resulting manifest an update for existing clients.

Recover through a separately authenticated installer distribution, such as a managed deployment channel or a release whose installer hash is verified using an independently trusted channel. An explicit manual reinstall can establish a new root, as with the sole test client above. Preserve the application ID, installation path and browser partition. Revoke/update affected credentials through the established incident process. A compromised bootstrap root can forge delegations for clients that have not yet received a newer packaged history; signed rotation cannot repair trust in a compromised root by itself. Require independently authenticated recovery when the trust anchor is compromised.

Never suppress signature failures, permit a downgrade, disable TLS checks or use an unverified manifest as a recovery mechanism. Windows Authenticode is separate and remains optional; endpoint and PowerShell execution policies are not bypassed.
