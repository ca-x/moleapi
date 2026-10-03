import { native } from "../../shared/api";
import type { ProtoFile } from "./types";
export const MAX_PROTO_BYTES = 2 * 1024 * 1024;
export const MAX_PROTO_FILE_BYTES = 256 * 1024;
export function checkedProtoFiles(files: ProtoFile[]): ProtoFile[] {
  if (!files.length || files.length > 32)
    throw new Error("选择 1–32 个 proto 文件。");
  const encoder = new TextEncoder();
  if (
    files.reduce(
      (sum, file) => sum + encoder.encode(file.content).byteLength,
      0,
    ) > MAX_PROTO_BYTES
  )
    throw new Error("Proto 文件内容合计不能超过 2 MiB。");
  if (
    files.some(
      (file) => encoder.encode(file.content).byteLength > MAX_PROTO_FILE_BYTES,
    )
  )
    throw new Error("每个 Proto 文件不能超过 256 KiB。");
  const seen = new Set<string>();
  for (const file of files) {
    const parts = file.path.split("/");
    if (
      !file.path.endsWith(".proto") ||
      file.path.includes("\\") ||
      file.path.includes(":") ||
      parts.some((part) => !part || part === "." || part === "..")
    )
      throw new Error("使用相对虚拟路径，例如 api/common.proto。");
    if (seen.has(file.path)) throw new Error(`重复路径：${file.path}`);
    seen.add(file.path);
  }
  return files;
}
/** Retain import-relative folders while discarding the user's absolute paths. */
export function relativeProtoPaths(paths: string[]): string[] {
  const parts = paths.map((path) => path.replaceAll("\\", "/").split("/"));
  const common = parts[0]?.slice(0, -1) || [];
  while (
    common.length &&
    !parts.every((path) => common.every((part, index) => path[index] === part))
  )
    common.pop();
  return parts.map((path) =>
    (common.length ? path.slice(common.length) : path.slice(-1)).join("/"),
  );
}
export async function pickProtoFiles(): Promise<ProtoFile[] | null> {
  if (native) {
    const [{ open }, { readTextFile }] = await Promise.all([
      import("@tauri-apps/plugin-dialog"),
      import("@tauri-apps/plugin-fs"),
    ]);
    const selected = await open({
      multiple: true,
      filters: [{ name: "Protocol Buffers", extensions: ["proto"] }],
    });
    if (!selected) return null;
    const paths = typeof selected === "string" ? [selected] : selected;
    if (paths.length > 32) throw new Error("一次最多导入 32 个 proto 文件。");
    const names = relativeProtoPaths(paths);
    return checkedProtoFiles(
      await Promise.all(
        paths.map(async (path, index) => ({
          path: names[index],
          content: await readTextFile(path),
        })),
      ),
    );
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
        return reject(new Error("最多 32 个文件，每个 256 KiB，合计 2 MiB。"));
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
