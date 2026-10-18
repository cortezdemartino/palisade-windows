'use strict';
const fs = require('node:fs/promises');
const { createReadStream } = require('node:fs');
const { createHash } = require('node:crypto');
const path = require('node:path');
const pinned = require('../assets/manager-release.json');
async function verifyManager(source, expected = pinned) {
  const details = await fs.stat(source).catch(() => null);
  if (!details?.isFile() || details.size !== expected.size) throw new Error('The pinned update manager is missing or has an unexpected size.');
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(source)) hash.update(chunk);
  if (hash.digest('hex') !== expected.sha256) throw new Error('Update manager SHA-256 verification failed.');
}
async function bundleManager(context) {
  const project = context.packager.projectDir;
  const source = process.env.PALISADE_MANAGER_EXE || path.join(project, '.manager', pinned.filename);
  await verifyManager(source);
  const destination = path.join(project, '.manager');
  await fs.mkdir(destination, { recursive: true });
  const target = path.join(destination, 'Palisade-Manager.exe');
  if (path.resolve(source) !== path.resolve(target)) await fs.copyFile(source, target);
  await verifyManager(target);
}
module.exports = bundleManager;
module.exports.verifyManager = verifyManager;
