import { readBoundedTextFiles } from "../../shared/readBoundedTextFiles";
import { native } from "../../shared/api";
import { relativeFilePaths } from "../../shared/relativeFilePaths";
import type { SoapSource } from "./types";
type SourceFile = SoapSource["files"][number];
export function checkSoapFiles(files: SourceFile[]): SourceFile[] {
  if (!files.length || files.length > 32)
    throw new Error("提供 1–32 个 WSDL/XSD 文件。");
  const encoder = new TextEncoder();
  if (
    files.reduce(
      (sum, file) => sum + encoder.encode(file.content).byteLength,
      0,
    ) >
    2 * 1024 * 1024
  )
    throw new Error("来源内容合计不能超过 2 MiB。");
  const seen = new Set<string>();
  for (const file of files) {
    if (
      !file.path ||
      file.path.length > 256 ||
      !/^[A-Za-z0-9_./-]+$/.test(file.path) ||
      file.path
        .split("/")
        .some((part) => !part || part === "." || part === "..")
    )
      throw new Error(
        "使用相对 ASCII 路径，例如 api/service.wsdl 或 types/common.xsd。",
      );
    if (seen.has(file.path)) throw new Error("重复路径：" + file.path);
    seen.add(file.path);
  }
  return files;
}
export async function pickSoapFiles(): Promise<SourceFile[] | null> {
  if (native) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      multiple: true,
      filters: [{ name: "WSDL / XSD", extensions: ["wsdl", "xsd", "xml"] }],
    });
    if (!selected) return null;
    const paths = typeof selected === "string" ? [selected] : selected;
    if (paths.length > 32) throw new Error("最多选择 32 个定义文件。");
    const names = relativeFilePaths(paths);
    const contents = await readBoundedTextFiles(paths, 2 * 1024 * 1024, 2 * 1024 * 1024);
    return checkSoapFiles(paths.map((_, index) => ({
      path: names[index], content: contents[index],
    })));
  }
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.accept = ".wsdl,.xsd,.xml";
    input.addEventListener("cancel", () => resolve(null));
    input.onchange = () => {
      const files = Array.from(input.files || []);
      if (!files.length) return resolve(null);
      if (
        files.length > 32 ||
        files.reduce((sum, file) => sum + file.size, 0) > 2 * 1024 * 1024
      )
        return reject(new Error("最多 32 个文件，合计 2 MiB。"));
      Promise.all(
        files.map(async (file) => ({
          path: file.webkitRelativePath || file.name,
          content: await file.text(),
        })),
      ).then((rows) => {
        try {
          resolve(checkSoapFiles(rows));
        } catch (error) {
          reject(error);
        }
      }, reject);
    };
    input.click();
  });
}
