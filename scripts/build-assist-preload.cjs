"use strict";
const fs = require("node:fs/promises");
const path = require("node:path");
async function buildAssistPreload() {
  const root = path.join(__dirname, "..", "src");
  const files = [
    "chat-notifications-preload.cjs",
    "assist/local-video-preload.cjs",
  ];
  const sources = await Promise.all(
    files.map((file) => fs.readFile(path.join(root, file), "utf8")),
  );
  const text =
    "// Palisade isolated hosted preload. Rebuilt from the two maintained sources.\n" +
    sources.join("\n;\n");
  await fs.writeFile(path.join(root, "assist/hosted-preload.cjs"), text);
}
module.exports = { buildAssistPreload };
if (require.main === module)
  buildAssistPreload().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
