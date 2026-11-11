'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { randomUUID } = require('node:crypto');

function readState(filename, limit) {
  const fd = fs.openSync(filename, 'r');
  try {
    if (!fs.fstatSync(fd).isFile()) throw Error('Invalid state file.');
    const bytes = Buffer.alloc(limit + 1);
    let length = 0;
    while (length < bytes.length) {
      const count = fs.readSync(fd, bytes, length, bytes.length - length, null);
      if (!count) break;
      length += count;
    }
    if (length > limit) throw Error('State file is too large.');
    return JSON.parse(bytes.subarray(0, length));
  } finally { fs.closeSync(fd); }
}

function writeState(filename, value) {
  fs.mkdirSync(path.dirname(filename), { recursive: true });
  const temporary = filename + '.' + randomUUID() + '.tmp';
  const fd = fs.openSync(temporary, 'wx', 0o600);
  try {
    try {
      fs.writeFileSync(fd, JSON.stringify(value));
      fs.fsyncSync(fd);
    } finally { fs.closeSync(fd); }
    fs.renameSync(temporary, filename);
  } catch (error) {
    try { fs.unlinkSync(temporary); } catch { /* Only the file opened above is eligible for cleanup. */ }
    throw error;
  }
}
module.exports = { readState, writeState };
