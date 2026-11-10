'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const asar = require('@electron/asar');
const archive = path.join(__dirname, '..', 'dist', 'win-unpacked', 'resources', 'app.asar');
const files = asar.listPackage(archive);
for (const filename of ['src/main.cjs', 'src/windows.cjs', 'src/config.cjs', 'src/preload.cjs', 'src/ui-protocol.cjs',
  'src/updates/release.cjs', 'src/updates/service.cjs', 'src/ui/index.html', 'src/ui/style.css', 'src/ui/renderer.js',
  'src/updates/key-trust.cjs', 'src/updates/notes.cjs', 'src/updates/notes-history.cjs', 'src/updates/downloader.cjs',
  'assets/update-key-transitions.json',
  'assets/update-public-key.pem', 'assets/branding/palisade-shield.png', 'assets/branding/palisade-wordmark.png', 'assets/branding/onyxops-logo.png']) {
  assert.ok(asar.extractFile(archive, path.join(...filename.split('/'))).equals(fs.readFileSync(path.join(__dirname, '..', filename))), 'Packaged source differs: ' + filename);
}
assert.equal(files.some(name => name.includes('node_modules')), false, 'Build dependencies must not ship in the application archive.');
assert.equal(files.some(name => /private.*key|\.pfx$|\.p12$/i.test(name)), false, 'Private key material must not ship.');
const packageInfo = JSON.parse(asar.extractFile(archive, 'package.json'));
assert.equal(packageInfo.name, '@onyxops/palisade-installer');
assert.equal(packageInfo.version, require('../package.json').version);
console.log('Packaged source, three branding assets, production public key and dependency separation verified.');
