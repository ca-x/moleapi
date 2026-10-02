import { Download, Upload } from "lucide-react";
import { ToolButton } from "../../shared/ui";
import { nav } from "./navigation";
import { useWorkbench } from "../workbench/context";

export default function NavigationRail() {
  const state = useWorkbench();
  const { draft, view, setView, setHistoryResponse, openModal } = state;
  return (
    <aside className="nav-rail" aria-label="模块导航">
      {nav.map((item) => (
        <button
          className={`rail-button ${view === item.id ? "active" : ""}`}
          aria-current={view === item.id ? "page" : undefined}
          key={item.id}
          onClick={() => {
            setView(item.id);
            setHistoryResponse(null);
          }}
        >
          <item.icon size={20} />
          <span>{item.label}</span>
        </button>
      ))}
      <div className="rail-bottom">
        <ToolButton label="导入 API 数据" onClick={() => openModal("import")}>
          <Upload size={18} />
        </ToolButton>
        <ToolButton
          label="导出工作区"
          disabled={!draft}
          onClick={() => openModal("export")}
        >
          <Download size={18} />
        </ToolButton>
      </div>
    </aside>
  );
}
