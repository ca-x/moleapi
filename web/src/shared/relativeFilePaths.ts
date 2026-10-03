/** Keep selected common import roots while omitting absolute user filesystem paths. */
export function relativeFilePaths(paths: string[]): string[] {
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
