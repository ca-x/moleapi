import { expect, it } from "vitest";
import { hostConfig, parseHostConfig } from "./config";
import { newRequest } from "../../shared/model";
it("preserves the exact selected host entry and unknown fields without executing it", () => {
  const entry = '{ "command": "/usr/bin/node", "args": ["server.js"], "env": { "TENANT": "private" }, "extension": { "keep": true } }';
  const values = parseHostConfig('{"mcpServers":{"fixture":' + entry + ',"remote":{"type":"http","url":"https://example.test/mcp","headers":{"Authorization":"Bearer private"}}}}');
  expect(values[0].config.config_source).toBe(entry);
  expect(values[0].config.env[0].secret).toBe(true);
  const request = { ...newRequest(), protocol: values[0].config };
  expect(JSON.parse(hostConfig(request, "fixture")).mcpServers.fixture.extension).toEqual({ keep: true });
  expect(values[1].config.transport).toBe("http");
});
it("rejects unsupported or ambiguous transports and non-string process arguments", () => {
  for (const config of [{ url:"http://example.test",type:"sse" }, { command:"node",url:"http://example.test" },{ command:"node",args:[4] },{ command:"node",env:{ TOKEN:4 } }]) {
    expect(() => parseHostConfig(JSON.stringify(config))).toThrow();
  }
});
it("uses UTF8 for Basic headers and reports bounded credentials", () => {
  const entry = parseHostConfig('{"url":"https://example.test/mcp"}')[0];
  const request = { ...newRequest(),protocol:entry.config,url:entry.url,auth:{kind:"basic" as const,token:"",username:"鼹鼠",password:"secret"} };
  const header = JSON.parse(hostConfig(request, "fixture")).mcpServers.fixture.headers.Authorization;
  expect(new TextDecoder().decode(Uint8Array.from(atob(header.slice(6)),char => char.charCodeAt(0)))).toBe("鼹鼠:secret");
});

it("rejects duplicate source fields before canonical settings can diverge from preserved text", () => {
  expect(() => parseHostConfig('{"mcpServers":{"fixture":{"command":"/one"},"fixture":{"command":"/two"}}}')).toThrow("重复字段");
  expect(() => parseHostConfig('{"command":"/one","env":{"TOKEN":"first","TOKEN":"second"}}')).toThrow("重复字段");
});
