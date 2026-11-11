'use strict';
const bundleManager = require('./bundle-manager.cjs');
const { buildAssist } = require('./build-assist.cjs');
const { buildAssistPreload } = require('./build-assist-preload.cjs');
const { buildUpdate } = require('./build-update.cjs');
module.exports = async context => {
  require('./release-notes.cjs').loadReleaseDraft(require('../package.json').version);
  await bundleManager(context);
  await buildAssist();
  await buildAssistPreload();
  await buildUpdate();
};
