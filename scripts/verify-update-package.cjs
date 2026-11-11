'use strict';
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { extractFile, listPackage } = require('@electron/asar');
const root = path.join(__dirname, '..');
const archive = path.join(process.argv[2] || path.join(root, 'out/win-unpacked/resources'), 'app.asar');
for (const relative of ['src/updates/release.cjs', 'src/updates/key-trust.cjs', 'src/updates/notes.cjs',
  'src/updates/notes-history.cjs', 'src/updates/updater.cjs', 'src/updates/native-helper.cjs',
  'src/updates/downloader.cjs', 'src/updates/helper-integrity.json', 'src/updates/state-file.cjs',
  'src/updates/journal.cjs', 'src/settings.cjs', 'src/main.cjs', 'src/window-controls-preload.cjs',
  'src/window-chrome.html', 'src/window-chrome.js', 'assets/update-public-key.pem', 'assets/update-key-transitions.json']) {
  assert.ok(extractFile(archive, path.normalize(relative)).equals(fs.readFileSync(path.join(root, relative))),
    'Packaged update source differs: ' + relative);
}
const entries = listPackage(archive).map((entry) => entry.replaceAll('\\', '/'));
assert.equal(entries.some((entry) => entry.endsWith('/updates/install.ps1')), false, 'Legacy weak installer helper must not ship.');
assert.equal(entries.some((entry) => /private.*\.pem$|\.(?:pfx|p12)$/i.test(entry)), false, 'Private signing material must not ship.');
const { verifyHelper, exchange, FILENAME, PROTOCOL } = require('../src/updates/native-helper.cjs');
const resourceRoot = path.dirname(archive);
const binary = path.join(resourceRoot, 'update', FILENAME);
const version = require('../package.json').version;
verifyHelper(binary, extractFile(archive, path.normalize('src/updates/helper-integrity.json')), version)
  .then(async () => {
    for (const name of ['THIRD-PARTY-NOTICES.txt', 'RUST-COPYRIGHT.html']) assert.ok(fs.statSync(path.join(resourceRoot, 'update', name)).size > 0);
    assert.deepEqual(await exchange(binary, ['--version']), { protocol: PROTOCOL, schema: 1, version });
    await assert.rejects(exchange(binary, ['--status', String(process.pid)]), /refused/);
    console.log('Packaged update sources, native helper integrity, trust anchors and isolated UI match the verified source.');
  }).catch(error => { console.error(error.message); process.exitCode = 1; });
