import { LocalizedError } from "../../shared/i18n/errors";
import { readBoundedTextFiles } from "../../shared/readBoundedTextFiles";
import { native } from "../../shared/api";
import type { ProtoFile } from "./types";
export const MAX_PROTO_BYTES = 2 * 1024 * 1024;
export const MAX_PROTO_FILE_BYTES = 256 * 1024;
export function checkedProtoFiles(files: ProtoFile[]): ProtoFile[] {
  if (!files.length || files.length > 32)
    throw new LocalizedError("选择 1–32 个 proto 文件。");
  const encoder = new TextEncoder();
  if (
    files.reduce(
      (sum, file) => sum + encoder.encode(file.content).byteLength,
      0,
    ) > MAX_PROTO_BYTES
  )
    throw new LocalizedError("Proto 文件内容合计不能超过 2 MiB。");
  if (
    files.some(
      (file) => encoder.encode(file.content).byteLength > MAX_PROTO_FILE_BYTES,
    )
  )
    throw new LocalizedError("每个 Proto 文件不能超过 256 KiB。");
  const seen = new Set<string>();
  for (const file of files) {
    const parts = file.path.split("/");
    if (
      !file.path.endsWith(".proto") ||
      file.path.includes("\\") ||
      file.path.includes(":") ||
      parts.some((part) => !part || part === "." || part === "..")
    )
      throw new LocalizedError("使用相对虚拟路径，例如 api/common.proto。");
    if (seen.has(file.path)) throw new LocalizedError("重复路径：{{value0}}", { value0: file.path });
    seen.add(file.path);
  }
  return files;
}
export { relativeFilePaths as relativeProtoPaths } from "../../shared/relativeFilePaths";
import { relativeFilePaths } from "../../shared/relativeFilePaths";
export async function pickProtoFiles(): Promise<ProtoFile[] | null> {
  if (native) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      multiple: true,
      filters: [{ name: "Protocol Buffers", extensions: ["proto"] }],
    });
    if (!selected) return null;
    const paths = typeof selected === "string" ? [selected] : selected;
    if (paths.length > 32) throw new LocalizedError("一次最多导入 32 个 proto 文件。");
    const names = relativeFilePaths(paths);
    const contents = await readBoundedTextFiles(paths, MAX_PROTO_BYTES, MAX_PROTO_FILE_BYTES);
    return checkedProtoFiles(paths.map((_, index) => ({
      path: names[index], content: contents[index],
    })));
  }
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".proto";
    input.multiple = true;
    input.addEventListener("cancel", () => resolve(null));
    input.onchange = () => {
      const files = Array.from(input.files || []);
      if (!files.length) return resolve(null);
      if (
        files.length > 32 ||
        files.some((file) => file.size > MAX_PROTO_FILE_BYTES) ||
        files.reduce((sum, file) => sum + file.size, 0) > MAX_PROTO_BYTES
      )
        return reject(new LocalizedError("最多 32 个文件，每个 256 KiB，合计 2 MiB。"));
      Promise.all(
        files.map(async (file) => ({
          path: file.webkitRelativePath || file.name,
          content: await file.text(),
        })),
      ).then((values) => {
        try {
          resolve(checkedProtoFiles(values));
        } catch (error) {
          reject(error);
        }
      }, reject);
    };
    input.click();
  });
}
