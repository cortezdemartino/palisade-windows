"use strict";
const test = require("node:test"),
  assert = require("node:assert/strict"),
  fs = require("node:fs"),
  path = require("node:path");
const { createHash, createPublicKey, verify } = require("node:crypto");
const { stripTypeScriptTypes } = require("node:module");
const fixtures = require("./fixtures/assist-native-v1.json"),
  provenance = require("./fixtures/assist-native-v1.provenance.json");
const runtime = require("../src/assist/native-protocol.cjs");
const source = fs.readFileSync(
  path.join(__dirname, "fixtures/assist-native-v1.web.ts"),
  "utf8",
);
const canonical = import(
  "data:text/javascript;base64," +
    Buffer.from(stripTypeScriptTypes(source, { mode: "transform" })).toString(
      "base64",
    )
);
test("native bridge contract and fixtures retain exact companion provenance", () => {
  assert.equal(provenance.commit, "8f9c2a45da7017ee36a9567f76413da53bd76075");
  for (const file of provenance.files) {
    const bytes = Buffer.from(
      fs
        .readFileSync(path.join(__dirname, "fixtures", file.local), "utf8")
        .replace(/\r\n/g, "\n"),
    );
    assert.equal(createHash("sha256").update(bytes).digest("hex"), file.sha256);
    assert.equal(
      createHash("sha1")
        .update("blob " + bytes.length + "\0")
        .update(bytes)
        .digest("hex"),
      file.gitBlob,
    );
  }
});
test("runtime and canonical native parsers agree on every shared request/result/event fixture", async () => {
  const web = await canonical;
  for (const api of [runtime, web]) {
    for (const message of fixtures.requests.valid)
      assert.deepEqual(api.parseRequest(JSON.stringify(message)), message);
    for (const entry of fixtures.requests.invalid)
      assert.throws(
        () => api.parseRequest(JSON.stringify(entry.message)),
        (error) => error.code === entry.error,
        entry.name,
      );
    for (const entry of fixtures.responses.valid)
      assert.deepEqual(
        api.parseResponse(JSON.stringify(entry.message), entry.op),
        entry.message,
      );
    for (const entry of fixtures.responses.invalid)
      assert.throws(
        () => api.parseResponse(JSON.stringify(entry.message), entry.op),
        (error) => error.code === entry.error,
        entry.name,
      );
    for (const message of fixtures.events.valid)
      assert.deepEqual(api.parseEvent(JSON.stringify(message)), message);
    for (const entry of fixtures.events.invalid)
      assert.throws(
        () => api.parseEvent(JSON.stringify(entry.message)),
        (error) => error.code === entry.error,
        entry.name,
      );
  }
});
test("native statement vectors verify only for their exact labelled purpose", () => {
  const key = createPublicKey({
    key: Buffer.concat([
      Buffer.from("302a300506032b6570032100", "hex"),
      Buffer.from(fixtures.endpointPublicKey, "base64url"),
    ]),
    type: "spki",
    format: "der",
  });
  for (const entry of fixtures.statements) {
    const statement = Buffer.from(entry.statement, "base64url"),
      signature = Buffer.from(entry.signature, "base64url");
    assert.equal(statement.toString(), entry.text);
    assert(verify(null, statement, key, signature));
    for (const other of fixtures.statements)
      if (other !== entry)
        assert.equal(
          verify(
            null,
            Buffer.from(other.statement, "base64url"),
            key,
            signature,
          ),
          false,
        );
  }
});
