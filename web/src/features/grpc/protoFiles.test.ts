import { describe, it, expect } from "vitest";
import {
  checkedProtoFiles,
  MAX_PROTO_BYTES,
  relativeProtoPaths,
} from "./protoFiles";
import { methodMode } from "./types";
import type { GrpcMethod } from "./types";
describe("portable proto import boundaries", () => {
  it("preserves relative virtual import paths and rejects escapes/ambiguous names", () => {
    const files = [
      {
        path: "service.proto",
        content: 'syntax="proto3"; import "api/common.proto";',
      },
      { path: "api/common.proto", content: 'syntax="proto3";' },
    ];
    expect(checkedProtoFiles(files)).toEqual(files);
    for (const path of [
      "../x.proto",
      "/x.proto",
      "api/../x.proto",
      "api//x.proto",
      "C:/x.proto",
      "api\\x.proto",
      "./x.proto",
    ])
      expect(() => checkedProtoFiles([{ path, content: "" }])).toThrow("相对");
    expect(() => checkedProtoFiles([files[0], files[0]])).toThrow("重复");
  });
  it("uses UTF8 byte budgets and bounds file count before import", () => {
    expect(() =>
      checkedProtoFiles([
        {
          path: "a.proto",
          content: "中".repeat(Math.ceil(MAX_PROTO_BYTES / 3)),
        },
      ]),
    ).toThrow("2 MiB");
    expect(() =>
      checkedProtoFiles(
        Array.from({ length: 33 }, (_, index) => ({
          path: `${index}.proto`,
          content: "",
        })),
      ),
    ).toThrow("32");
  });
});
it("distinguishes all four actual descriptor-defined gRPC modes", () => {
  const method: GrpcMethod = {
    name: "Echo",
    full_name: "Service.Echo",
    input_type: "Input",
    output_type: "Output",
    input_template: {},
    client_streaming: false,
    server_streaming: false,
  };
  expect(methodMode(method)).toBe("Unary");
  expect(methodMode({ ...method, server_streaming: true })).toBe("服务端流");
  expect(methodMode({ ...method, client_streaming: true })).toBe("客户端流");
  expect(
    methodMode({ ...method, client_streaming: true, server_streaming: true }),
  ).toBe("双向流");
});

it("native multi-file selection preserves common import roots without storing absolute user paths", () => {
  expect(
    relativeProtoPaths([
      "/home/user/protos/service.proto",
      "/home/user/protos/api/common.proto",
    ]),
  ).toEqual(["service.proto", "api/common.proto"]);
  expect(
    relativeProtoPaths([
      "C:\\Users\\user\\protos\\service.proto",
      "C:\\Users\\user\\protos\\api\\common.proto",
    ]),
  ).toEqual(["service.proto", "api/common.proto"]);
});

it("rejects an oversized individual proto even below the bundle budget", () => {
  expect(() =>
    checkedProtoFiles([
      { path: "large.proto", content: "x".repeat(256 * 1024 + 1) },
    ]),
  ).toThrow("256 KiB");
});
