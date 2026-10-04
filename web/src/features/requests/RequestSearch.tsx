import { t, useLanguage } from "../../shared/i18n";
import { requestLabel } from "../../shared/model";
import { Button, Text, TextField } from "@radix-ui/themes";
import { Search } from "lucide-react";
import { useWorkbench } from "../workbench/context";
export default function RequestSearch() {
  useLanguage();
  const {
    draft,
    content,
    setContent,
    setRequestId,
    setView,
    setResponse,
    setModal,
  } = useWorkbench();
  return (
    <>
      <TextField.Root
        autoFocus
        aria-label={t("搜索请求")}
        placeholder={t("名称、方法或 URL…")}
        value={content}
        onChange={(e) => setContent(e.target.value)}
      >
        <TextField.Slot>
          <Search size={16} />
        </TextField.Slot>
      </TextField.Root>
      <div className="search-results">
        {draft?.data.collections
          .flatMap((c) => c.requests)
          .filter((r) =>
            `${r.name} ${requestLabel(r)} ${r.url}`
              .toLowerCase()
              .includes(content.toLowerCase()),
          )
          .map((r) => (
            <Button
              key={r.id}
              color="gray"
              variant="ghost"
              onClick={() => {
                setRequestId(r.id);
                setView("requests");
                setResponse();
                setModal(null);
              }}
            >
              <span
                className={`method method-${requestLabel(r).toLowerCase()}`}
              >
                {requestLabel(r)}
              </span>
              {r.name}
              <Text size="1" color="gray" className="truncate">
                {r.url}
              </Text>
            </Button>
          ))}
      </div>
    </>
  );
}
