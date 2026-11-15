'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { stripTypeScriptTypes } = require('node:module');
const root = path.resolve(__dirname, '..');
const source = fs.readFileSync(path.join(root, 'tests/fixtures/assist-native-ext-v2.web.ts'), 'utf8');
const exportedNames = [...source.matchAll(/^export (?:const|class|function) (\w+)/gm)].map(match => match[1]);
const code = stripTypeScriptTypes(source, { mode: 'transform' })
  .replace(/import \{ NATIVE_LIMITS \} from ['"]\.\/protocol['"];?/, "const { NATIVE_LIMITS } = require('./native-protocol.cjs');")
  .replace(/\bexport (?=const|class|function)/g, '');
const output = "'use strict';\n// Canonical native extension validation (frozen companion extension.ts).\n" + code +
  '\nmodule.exports = Object.freeze({' + exportedNames.join(',') + '});\n';
const target = path.join(root, 'src/assist/native-extension.cjs');
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8').replace(/\r\n/g, '\n') !== output.replace(/\r\n/g, '\n'))
    throw Error('Native extension port differs from the frozen source');
} else fs.writeFileSync(target, output);
