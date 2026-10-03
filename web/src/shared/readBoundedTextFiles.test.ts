import { beforeEach, expect, it, vi } from "vitest";
import { readBoundedTextFiles } from "./readBoundedTextFiles";
const fs = vi.hoisted(() => ({ stat: vi.fn(), open: vi.fn() }));
vi.mock("@tauri-apps/plugin-fs", () => fs);
beforeEach(() => { fs.stat.mockReset(); fs.open.mockReset(); });
it("rejects oversized selections before opening any file", async () => {
  fs.stat.mockResolvedValue({ isFile: true, size: 3_000_000 });
  await expect(readBoundedTextFiles(["large.wsdl"], 2_097_152, 2_097_152)).rejects.toThrow("大小限制");
  expect(fs.open).not.toHaveBeenCalled();
});
it("bounds a file that grows after preflight and closes its handle", async () => {
  fs.stat.mockResolvedValue({ isFile: true, size: 1 });
  const close = vi.fn();
  fs.open.mockResolvedValue({ read: async (buffer: Uint8Array) => buffer.length, close });
  await expect(readBoundedTextFiles(["growing.xsd"], 4, 4)).rejects.toThrow("大小限制");
  expect(close).toHaveBeenCalledOnce();
});
it("preserves UTF8 characters and the source BOM split across chunks and enforces the aggregate budget", async () => {
  fs.stat.mockResolvedValue({ isFile: true, size: 3 });
  const contents = [new TextEncoder().encode("\ufeff中"), new TextEncoder().encode("文")];
  const close = vi.fn();
  fs.open.mockImplementation(async () => {
    const bytes = contents.shift()!;
    let offset = 0;
    return { close, read: async (buffer: Uint8Array) => {
      if (offset === bytes.length) return null;
      buffer[0] = bytes[offset++];
      return 1;
    }};
  });
  await expect(readBoundedTextFiles(["one.wsdl", "two.xsd"], 9, 6)).resolves.toEqual(["\ufeff中", "文"]);
  expect(close).toHaveBeenCalledTimes(2);
});
