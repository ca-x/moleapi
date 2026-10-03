import { useEffect } from "react";
import { toast } from "sonner";
import { native } from "../../shared/api";

/** Protect drafts when closing either the browser tab or a native window. */
export function useCloseGuard(hasChanges: () => boolean) {
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (hasChanges()) {
        event.preventDefault();
        event.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", beforeUnload);
    let disposed = false;
    let asking = false;
    let unlisten: (() => void) | undefined;
    if (native) {
      void Promise.all([
        import("@tauri-apps/api/window"),
        import("@tauri-apps/plugin-dialog"),
      ])
        .then(async ([{ getCurrentWindow }, { ask }]) => {
          if (disposed) return;
          const currentWindow = getCurrentWindow();
          const cleanup = await currentWindow.onCloseRequested(
            async (event) => {
              if (!hasChanges()) return;
              event.preventDefault();
              if (asking) return;
              asking = true;
              try {
                const discard = await ask(
                  "工作区有未保存的修改。关闭窗口将丢弃这些修改。",
                  {
                    title: "关闭 MoleAPI",
                    kind: "warning",
                    okLabel: "丢弃并关闭",
                    cancelLabel: "继续编辑",
                  },
                );
                if (discard && !disposed) await currentWindow.destroy();
              } finally {
                asking = false;
              }
            },
          );
          if (disposed) cleanup();
          else unlisten = cleanup;
        })
        .catch(() => {
          if (!disposed)
            toast.error("无法启用窗口关闭确认，请先保存工作区再关闭窗口。");
        });
    }
    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("beforeunload", beforeUnload);
    };
  }, [hasChanges]);
}
