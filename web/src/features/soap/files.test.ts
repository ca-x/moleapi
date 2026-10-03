import { describe, expect, it } from "vitest";
import { checkSoapFiles } from "./files";
describe("portable WSDL/XSD bundles", () => {
  it("retains import-relative names and raw XML, rejects ambiguous/path escapes", () => {
    const files = [
      {
        path: "service.wsdl",
        content: '<definitions xmlns="http://schemas.xmlsoap.org/wsdl/"/>',
      },
      {
        path: "types/model.xsd",
        content: '<schema xmlns="http://www.w3.org/2001/XMLSchema"/>',
      },
    ];
    expect(checkSoapFiles(files)).toEqual(files);
    for (const path of [
      "../service.wsdl",
      "/service.wsdl",
      "types/../model.xsd",
      "C:/service.wsdl",
      "types\\model.xsd",
      "types//model.xsd",
    ])
      expect(() => checkSoapFiles([{ path, content: "" }])).toThrow("相对");
    expect(() => checkSoapFiles([files[0], files[0]])).toThrow("重复");
  });
  it("limits UTF8 bundle bytes and file counts before source upload", () => {
    expect(() =>
      checkSoapFiles([{ path: "source.wsdl", content: "中".repeat(700000) }]),
    ).toThrow("2 MiB");
    expect(() =>
      checkSoapFiles(
        Array.from({ length: 33 }, (_, index) => ({
          path: "source" + index + ".xsd",
          content: "",
        })),
      ),
    ).toThrow("32");
  });
});
