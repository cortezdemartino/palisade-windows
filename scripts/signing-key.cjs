'use strict';
const { readFileSync, realpathSync } = require('node:fs');
const { createPrivateKey } = require('node:crypto');
const path = require('node:path');

function loadPrivateKey(filename) {
  if (!filename) throw new Error('An external signing-key path is required.');
  const relative = path.relative(realpathSync(path.join(__dirname, '..')), realpathSync(filename));
  if (!relative || (!path.isAbsolute(relative) && relative !== '..' && !relative.startsWith(`..${path.sep}`))) {
    throw new Error('Private signing keys must remain outside the source repository.');
  }
  const key = createPrivateKey(readFileSync(filename));
  if (key.asymmetricKeyType !== 'ed25519') throw new Error('Expected an Ed25519 private key.');
  return key;
}
module.exports = { loadPrivateKey };
