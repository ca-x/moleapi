import { LocalizedError } from "./i18n/errors";
/** Read dialog-selected native sources with a byte budget even if a file grows. */
export async function readBoundedTextFiles(
  paths: string[],
  maxTotal: number,
  maxFile: number,
): Promise<string[]> {
  const { stat, open } = await import("@tauri-apps/plugin-fs");
  const infos = await Promise.all(paths.map(path => stat(path)));
  if (infos.some(info => !info.isFile || info.size > maxFile) ||
      infos.reduce((total, info) => total + info.size, 0) > maxTotal)
    throw new LocalizedError("定义文件超过大小限制。");
  const values: string[] = [];
  let remaining = maxTotal;
  for (const path of paths) {
    const file = await open(path, { read: true });
    try {
      const budget = Math.min(remaining, maxFile);
      const buffer = new Uint8Array(Math.min(65536, budget + 1));
      const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
      const chunks: string[] = [];
      let bytes = 0;
      for (;;) {
        const count = await file.read(buffer.subarray(0, Math.min(buffer.length, budget - bytes + 1)));
        if (!count) break;
        bytes += count;
        if (bytes > budget) throw new LocalizedError("定义文件超过大小限制。");
        chunks.push(decoder.decode(buffer.subarray(0, count), { stream: true }));
      }
      chunks.push(decoder.decode());
      remaining -= bytes;
      values.push(chunks.join(""));
    } finally {
      await file.close();
    }
  }
  return values;
}
