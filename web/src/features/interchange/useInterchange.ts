import { useLanguage, liveTranslation } from "../../shared/i18n";
import { useState } from "react";
import { toast } from "sonner";
import { api, saveFile } from "../../shared/api";
import type { ExportResult, ImportResult, Workspace } from "../../shared/types";
import { hasChanges } from "../workspaces/draft";
import type { useWorkspace } from "../workspaces/useWorkspace";
export function useInterchange(workspace: ReturnType<typeof useWorkspace>) {
  useLanguage();
  const [format, setFormat] = useState("openapi");
  const [content, setContent] = useState("");
  const [includeSecrets, setIncludeSecrets] = useState(false);
  async function importWorkspace() {
    const imported = await api<ImportResult>("/api/import", "POST", {
      format,
      content,
    });
    workspace.installWorkspace(
      await api<Workspace>("/api/workspaces", "POST", {
        name: imported.name,
        data: imported.data,
      }),
    );
    if (imported.warnings.length)
      toast.message(liveTranslation("导入说明"), {
        description: imported.warnings.join("；"),
        duration: 10000,
      });
  }
  async function exportWorkspace() {
    const current = workspace.draft;
    if (!current) return false;
    if (workspace.dirty && !(await workspace.save(true))) return false;
    if (hasChanges(workspace.stateRef.current)) {
      toast.message(liveTranslation("保存期间有新修改，请保存后再次导出。"));
      return false;
    }
    const result = await api<ExportResult>(
      `/api/workspaces/${current.id}/export`,
      "POST",
      { format, include_secrets: includeSecrets },
    );
    await saveFile(result);
    return true;
  }
  return {
    format,
    setFormat,
    content,
    setContent,
    includeSecrets,
    setIncludeSecrets,
    importWorkspace,
    exportWorkspace,
  };
}
