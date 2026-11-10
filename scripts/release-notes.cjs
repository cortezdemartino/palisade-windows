'use strict';

const { readFileSync, statSync } = require('node:fs');
const path = require('node:path');
const { normaliseDraft, NOTES_LIMIT } = require('../src/updates/notes.cjs');

function loadReleaseDraft(version, filename = path.join(__dirname, '..', 'release-notes', `${version}.json`)) {
  if (statSync(filename).size > NOTES_LIMIT) throw new Error('Editorial release notes are too large.');
  const draft = normaliseDraft(JSON.parse(readFileSync(filename, 'utf8')), version);
  if (/Replace this text/i.test(JSON.stringify(draft))) throw new Error('Replace the editorial template before release.');
  return draft;
}

if (require.main === module) {
  try {
    const version = require('../package.json').version;
    loadReleaseDraft(version, process.argv[2]);
    console.log(`Validated editorial notes for Palisade ${version}.`);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = { loadReleaseDraft };
