import { t, message, translateCopy } from "./i18n";
import type { LocalizedCopy } from "./i18n";
import { AppError, LocalizedError } from "./i18n/errors";
import type { ExportResult } from "./types";
export const native =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export class ApiError extends AppError {
  status: number;
  constructor(message: string | LocalizedCopy, status: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}
export function token(): string | null {
  return sessionStorage.getItem("moleapi_token");
}
export async function api<T>(
  path: string,
  method = "GET",
  body?: unknown,
): Promise<T> {
  let status: number, data: unknown;
  let parseFailure: LocalizedCopy | undefined;
  const requestToken = native ? null : token();
  if (native) {
    const { invoke } = await import("@tauri-apps/api/core");
    const reply = await invoke<{ status: number; body: unknown }>("api", {
      method,
      path,
      body: body ?? null,
    });
    status = reply.status;
    data = reply.body;
  } else {
    const auth = requestToken;
    const response = await fetch(path, {
      method,
      headers: {
        "Content-Type": "application/json",
        ...(auth ? { Authorization: `Bearer ${auth}` } : {}),
      },
      ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
    });
    status = response.status;
    data = await response
      .json()
      .catch(() => {
        parseFailure = message("服务返回了无法解析的数据 ({{value0}})", { value0: status });
        return { error: translateCopy(parseFailure) };
      });
  }
  if (status < 200 || status >= 300) {
    if (
      status === 401 &&
      !native &&
      requestToken === token() &&
      !path.startsWith("/api/auth/")
    )
      window.dispatchEvent(new Event("moleapi:unauthorized"));
    const serverError = (data as { error?: string })?.error;
    throw new ApiError(
      parseFailure || (serverError ? String(serverError) : message("请求失败 ({{value0}})", { value0: status })),
      status,
    );
  }
  return data as T;
}
export async function saveFile(file: ExportResult): Promise<void> {
  if (native) {
    const [{ save }, { writeTextFile }] = await Promise.all([
      import("@tauri-apps/plugin-dialog"),
      import("@tauri-apps/plugin-fs"),
    ]);
    const path = await save({
      defaultPath: file.filename,
      filters: [{ name: t("API 文件"), extensions: ["json", "yaml", "txt"] }],
    });
    if (path) await writeTextFile(path, file.content);
    return;
  }
  const url = URL.createObjectURL(
    new Blob([file.content], { type: file.mime }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = file.filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export async function pickFile(): Promise<string | null> {
  if (native) {
    const [{ open }, { readTextFile }] = await Promise.all([
      import("@tauri-apps/plugin-dialog"),
      import("@tauri-apps/plugin-fs"),
    ]);
    const path = await open({
      multiple: false,
      filters: [
        { name: t("API 定义"), extensions: ["json", "yaml", "yml", "txt"] },
      ],
    });
    return typeof path === "string" ? readTextFile(path) : null;
  }
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json,.yaml,.yml,.txt";
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return resolve(null);
      if (file.size > 5 * 1024 * 1024) {
        reject(new LocalizedError("文件超过 5 MiB 导入限制。"));
        return;
      }
      file.text().then(resolve, reject);
    };
    input.addEventListener("cancel", () => resolve(null));
    input.click();
  });
}
