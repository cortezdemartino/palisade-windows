'use strict';

const { createHash, timingSafeEqual } = require('node:crypto');
const { open, rm } = require('node:fs/promises');
const path = require('node:path');
const { fetchAsset, releaseAssetUrl } = require('./release.cjs');

async function downloadInstaller(manifest, destination, fetchFn = fetch, onProgress = () => {}) {
  const filename = path.join(destination, manifest.filename);
  const response = await fetchAsset(releaseAssetUrl(manifest.version, manifest.filename), {
    fetchFn, signal: AbortSignal.timeout(10 * 60 * 1000),
  });
  let fd;
  let bytes = 0;
  const hash = createHash('sha512');
  const sha256 = createHash('sha256');
  let created = false;
  try {
    fd = await open(filename, 'wx', 0o600);
    created = true;
    for await (const part of response.body) {
      bytes += part.length;
      if (bytes > manifest.size) throw new Error('Installer is larger than the signed size.');
      hash.update(part);
      sha256.update(part);
      let offset = 0;
      while (offset < part.length) {
        const { bytesWritten } = await fd.write(part, offset, part.length - offset, null);
        if (bytesWritten <= 0) throw new Error('Installer write did not complete.');
        offset += bytesWritten;
      }
      onProgress(bytes, manifest.size);
    }
    await fd.sync();
    await fd.close();
    fd = undefined;
    const actual = hash.digest();
    const expected = Buffer.from(manifest.sha512, 'base64');
    if (bytes !== manifest.size || actual.length !== expected.length ||
        !timingSafeEqual(actual, expected) || (manifest.schema >= 2 && sha256.digest('hex') !== manifest.sha256)) {
      throw new Error('Installer does not match the signed manifest.');
    }
    return filename;
  } catch (error) {
    if (fd) await fd.close();
    if (created) await rm(filename, { force: true });
    throw error;
  }
}

module.exports = { downloadInstaller };
