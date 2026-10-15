'use strict';

const { createHash, timingSafeEqual } = require('node:crypto');
const { open, rm } = require('node:fs/promises');
const path = require('node:path');
const { fetchAsset, releaseAssetUrl } = require('./release.cjs');

async function downloadInstaller(manifest, destination, fetchFn = fetch) {
  const filename = path.join(destination, manifest.filename);
  const response = await fetchAsset(releaseAssetUrl(manifest.version, manifest.filename), {
    fetchFn, signal: AbortSignal.timeout(10 * 60 * 1000),
  });
  let fd;
  let bytes = 0;
  const hash = createHash('sha512');
  try {
    fd = await open(filename, 'wx', 0o600);
    for await (const part of response.body) {
      bytes += part.length;
      if (bytes > manifest.size) throw new Error('Installer is larger than the signed size.');
      hash.update(part);
      let offset = 0;
      while (offset < part.length) {
        const { bytesWritten } = await fd.write(part, offset, part.length - offset, null);
        if (bytesWritten <= 0) throw new Error('Installer write did not complete.');
        offset += bytesWritten;
      }
    }
    await fd.sync();
    await fd.close();
    fd = undefined;
    const actual = hash.digest();
    const expected = Buffer.from(manifest.sha512, 'base64');
    if (bytes !== manifest.size || actual.length !== expected.length ||
        !timingSafeEqual(actual, expected)) {
      throw new Error('Installer does not match the signed manifest.');
    }
    return filename;
  } catch (error) {
    if (fd) await fd.close();
    await rm(filename, { force: true });
    throw error;
  }
}

module.exports = { downloadInstaller };
