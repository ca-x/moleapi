#!/usr/bin/env node
// Independent compatibility fixture. Libraries own request/auth/variable execution.
// npm install --prefix /tmp/moleapi-postman-runtime-qa --ignore-scripts --no-audit --no-fund postman-runtime@7.56.1 postman-collection@5.3.0
// node scripts/verify_postman_inheritance.cjs /tmp/moleapi-postman-runtime-qa/node_modules
const assert = require('node:assert/strict');
const http = require('node:http');
const path = require('node:path');
const root = path.resolve(process.argv[2] || 'node_modules');
const { Collection } = require(path.join(root, 'postman-collection'));
const { Runner } = require(path.join(root, 'postman-runtime'));
const versions = {
  runtime: require(path.join(root, 'postman-runtime/package.json')).version,
  sdk: require(path.join(root, 'postman-collection/package.json')).version,
};
assert.equal(versions.runtime, '7.56.1');
assert.equal(versions.sdk, '5.3.0');
const server = http.createServer((request, response) => {
  response.setHeader('Content-Type', 'application/json');
  response.end(JSON.stringify({ authorization: request.headers.authorization || null }));
});
(async () => {
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${server.address().port}/echo`;
  const source = {
    info: { name: 'MoleAPI compatibility fixture', schema: 'https://schema.getpostman.com/json/collection/v2.1.0/collection.json' },
    auth: { type: 'bearer', bearer: [{ key: 'token', value: '{{credential}}', type: 'string' }] },
    variable: [{ key: 'credential', value: 'root-runtime-secret' }],
    item: [{ name: 'Folder', variable: [{ key: 'credential', value: 'ignored-folder-definition' }], item: [
      { name: 'Inherited', request: { method: 'GET', url, auth: null } },
      { name: 'Anonymous', request: { method: 'GET', url, auth: { type: 'noauth' } } },
    ] }],
  };
  let requests = 0;
  await new Promise((resolve, reject) => new Runner().run(new Collection(source), { iterationCount: 1, timeout: { request: 3000 } }, (error, run) => {
    if (error) return reject(error);
    run.start({
      request: (error, cursor, response, request, item) => {
        if (error) return reject(error);
        try {
          const body = JSON.parse(response.stream.toString());
          assert.equal(body.authorization, item.name === 'Inherited' ? 'Bearer root-runtime-secret' : null);
          requests++;
        } catch (error) { reject(error); }
      },
      done: (error) => error ? reject(error) : resolve(),
    });
  }));
  assert.equal(requests, 2);
  console.log(JSON.stringify({ ...versions, requests, auth_inheritance_verified: true, folder_variables_ignored: true }));
})().catch((error) => { console.error(error); process.exitCode = 1; }).finally(() => server.close());
