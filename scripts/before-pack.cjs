'use strict';
const bundleManager = require('./bundle-manager.cjs');
const { buildAssist } = require('./build-assist.cjs');
const { buildAssistPreload } = require('./build-assist-preload.cjs');
module.exports = async context => {
  await bundleManager(context);
  await buildAssist();
  await buildAssistPreload();
};
