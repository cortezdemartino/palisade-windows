"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { createHash, createPublicKey, verify } = require("node:crypto");
const { stripTypeScriptTypes } = require("node:module");
const root = path.join(__dirname, "fixtures");
const fixtures = require("./fixtures/assist-native-ext-v1.json");
const provenance = require("./fixtures/assist-native-ext-v1.provenance.json");
const dataUrl = (text) => "data:text/javascript;base64," +
  Buffer.from(stripTypeScriptTypes(text, { mode: "transform" })).toString("base64");
const extensionSource = fs.readFileSync(path.join(root, "assist-native-ext-v1.web.ts"), "utf8");
const firstImport = "import { NATIVE_LIMITS } from './protocol';";
if (!extensionSource.startsWith(firstImport)) {
  throw Error("Pinned extension import changed: review contract before updating");
}
const extension = import(dataUrl(extensionSource.replace(
  firstImport, "const NATIVE_LIMITS = Object.freeze({ messageChars: 32_768 });"
)));
const control = import(dataUrl(fs.readFileSync(
  path.join(root, "assist-control-profile2.web.ts"), "utf8"
)));
const errorCode = (error, code) => error && error.code === code;

test("canonical native-extension source and fixtures retain exact web Git blob provenance", () => {
  assert.equal(provenance.commit, "82ac7de4ec2bbd5cc2fd48f3d15dbde6ea5fd375");
  for (const file of provenance.files) {
    const bytes = Buffer.from(fs.readFileSync(path.join(root, file.local), "utf8").replace(/\r\n/g, "\n"));
    assert.equal(createHash("sha1").update("blob " + bytes.length + "\0").update(bytes).digest("hex"), file.gitBlob, file.source);
  }
});

test("web reference accepts all canonical native-extension requests, responses and events", async () => {
  const api = await extension;
  for (const m of fixtures.requests.valid) assert.deepEqual(api.parseExtRequest(JSON.stringify(m)), m);
  for (const r of fixtures.responses.valid) assert.deepEqual(api.parseExtResponse(JSON.stringify(r.message), r.op), r.message);
  for (const m of fixtures.events.valid) assert.deepEqual(api.parseExtEvent(JSON.stringify(m)), m);
  assert.equal(api.protocolOf(JSON.stringify(fixtures.requests.valid[0])), "ext");
  assert.equal(api.protocolOf('{"p":"palisade.assist.native","v":1}'), "native");
  assert.equal(api.protocolOf("bad json"), undefined);
});

test("web reference rejects all malformed native-extension requests, responses and events", async () => {
  const api = await extension;
  for (const r of fixtures.requests.invalid) assert.throws(() => api.parseExtRequest(JSON.stringify(r.message)), error => errorCode(error, r.error), r.name);
  for (const r of fixtures.responses.invalid) assert.throws(() => api.parseExtResponse(JSON.stringify(r.message), r.op), error => errorCode(error, r.error), r.name);
  for (const r of fixtures.events.invalid) assert.throws(() => api.parseExtEvent(JSON.stringify(r.message)), error => errorCode(error, r.error), r.name);
});

test("resource data-plane fixtures use the current canonical v2 parser and reject invalid cases", async () => {
  const api = await control;
  for (const m of fixtures.dataPlane.valid) assert.deepEqual(api.parseControlMessage(m, 2), m);
  for (const r of fixtures.dataPlane.invalid) assert.throws(() => api.parseControlMessage(r.message, r.version), error => errorCode(error, r.error), r.name);
});

test("canonical relay signing fixture verifies on original bytes only", () => {
  const key = createPublicKey({
    key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), Buffer.from(fixtures.endpointPublicKey, "base64url")]),
    type: "spki", format: "der"
  });
  for (const r of fixtures.statements) {
    const statement = Buffer.from(r.statement, "base64url");
    const signature = Buffer.from(r.signature, "base64url");
    assert.equal(statement.toString("utf8"), r.text);
    assert.equal(verify(null, statement, key, signature), true, r.name);
    assert.equal(verify(null, Buffer.concat([statement, Buffer.from("x")]), key, signature), false);
  }
});

// Contract conformance alone does not establish a Windows resource runtime,
// operating-system authority or live web-to-Windows session. Keep gates off.
