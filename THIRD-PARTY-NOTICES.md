# Third-party notices

The packaged Palisade Windows client includes Electron, Chromium and Node.js, whose licenses and
third-party notices are distributed in the packaged Electron runtime (including `LICENSE` and
`LICENSES.chromium.html`). Electron is MIT-licensed: https://github.com/electron/electron/blob/main/LICENSE.

The Palisade interface, Matrix SDK and cryptography are served by the Palisade installation,
not incorporated into the Electron package. Their deployment-specific notices continue to apply.

Build toolchain dependency licenses are provided with their packages in node_modules during builds.
Do not strip Electron runtime licensing files when distributing an installer.

Window control SVGs are from Tabler Icons (https://github.com/tabler/tabler-icons),
Copyright (c) 2020-2026 Paweł Kuna. Their MIT license is included in
`src/icons/TABLER-LICENSE`; the pinned source revision is recorded in `src/icons/README.md`.

The inactive Palisade Assist guardian is built with Rust. Its locked dependency licenses and
copyright notices are included under `resources/assist/THIRD-PARTY-NOTICES.txt`,
`RUST-COPYRIGHT.html` and `licenses/`. They include the BSD-licensed dalek cryptography
libraries and the MIT, Apache and Unicode notices of their dependencies. The build fails if
required dependency or Rust runtime license material is missing. Do not remove these files.
