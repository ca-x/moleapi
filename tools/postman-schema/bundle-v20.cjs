#!/usr/bin/env node
// Bundle the pinned Postman 2.0 draft-04 source schema without runtime network access.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const json5 = require(path.resolve(__dirname, "../../web/node_modules/json5"));

const source = process.argv[2];
const output = process.argv[3] || path.resolve(__dirname, "../../crates/formats/schemas/postman20.json");
if (!source) {
  process.stderr.write("Usage: bundle-v20.cjs <pinned-upstream/schemas/draft-04/v2.0.0> [output]\n");
  process.exit(2);
}
const root = json5.parse(fs.readFileSync(path.join(source, "collection.json"), "utf8"));
root.definitions = {};
for (const name of fs.readdirSync(path.join(source, "collection")).sort()) {
  if (name.endsWith(".json")) {
    root.definitions[name.slice(0, -5)] = json5.parse(fs.readFileSync(path.join(source, "collection", name), "utf8"));
  }
}
if (Object.keys(root.definitions).length !== 22) throw new Error("Expected 22 Postman 2.0 definitions");
const bytes = JSON.stringify(root, null, 2) + "\n";
const digest = crypto.createHash("sha256").update(bytes).digest("hex");
const expected = "24ab6a451edfc985b6db44a046c965aacc026a275a7832af5b79707650bc3faf";
if (digest !== expected) throw new Error(`Pinned Postman 2.0 schema mismatch: ${digest}`);
fs.writeFileSync(output, bytes);
process.stdout.write(`${digest}  ${output}\n`);
