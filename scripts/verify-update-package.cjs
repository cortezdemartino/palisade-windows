'use strict';
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { extractFile, listPackage } = require('@electron/asar');
const root = path.join(__dirname, '..');
const archive = path.join(process.argv[2] || path.join(root, 'out/win-unpacked/resources'), 'app.asar');
for (const relative of ['src/updates/release.cjs', 'src/updates/key-trust.cjs', 'src/updates/notes.cjs',
  'src/updates/notes-history.cjs', 'src/updates/updater.cjs', 'src/window-controls-preload.cjs',
  'src/window-chrome.html', 'src/window-chrome.js', 'assets/update-public-key.pem', 'assets/update-key-transitions.json']) {
  assert.ok(extractFile(archive, path.normalize(relative)).equals(fs.readFileSync(path.join(root, relative))),
    'Packaged update source differs: ' + relative);
}
const entries = listPackage(archive).map((entry) => entry.replaceAll('\\', '/'));
assert.equal(entries.some((entry) => entry.endsWith('/updates/install.ps1')), false, 'Legacy weak installer helper must not ship.');
assert.equal(entries.some((entry) => /private.*\.pem$|\.(?:pfx|p12)$/i.test(entry)), false, 'Private signing material must not ship.');
console.log('Packaged signed-notes validators, trust anchors and isolated UI match the verified source.');
