# Third-party notices

The packaged Palisade Windows client includes Electron, Chromium and Node.js, whose licenses and
third-party notices are distributed in the packaged Electron runtime (including `LICENSE` and
`LICENSES.chromium.html`). Electron is MIT-licensed: https://github.com/electron/electron/blob/main/LICENSE.

The Palisade interface, Matrix SDK and cryptography are served by the Palisade installation,
not incorporated into the Electron package. Their deployment-specific notices continue to apply.

Build toolchain dependency licenses are provided with their packages in node_modules during builds.
Do not strip Electron runtime licensing files when distributing an installer.
