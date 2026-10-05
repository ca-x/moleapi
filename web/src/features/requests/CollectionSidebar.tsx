import { useState, useEffect } from "react";
import CollectionSettingsDialog from "../collections/CollectionSettingsDialog";
import type { CollectionSettingsTarget } from "../collections/CollectionSettingsDialog";
import { collectionRows, removeCollectionTree } from "../collections/tree";
import { t, useLanguage, message } from "../../shared/i18n";
import { requestLabel } from "../../shared/model";
import {
  Button,
  DropdownMenu,
  Flex,
  ScrollArea,
  Text,
  TextField,
} from "@radix-ui/themes";
import {
  ChevronDown,
  ChevronRight,
  Folder,
  MoreHorizontal,
  Plus,
  Search,
  Upload,
} from "lucide-react";
import { ToolButton } from "../../shared/ui";
import { id } from "../../shared/model";
import { useWorkbench } from "../workbench/context";

export default function CollectionSidebar() {
  useLanguage();
  const state = useWorkbench();
  const [target,setTarget] = useState<CollectionSettingsTarget | null>(null);
  const [collapsed,setCollapsed]=useState(new Set<string>());
  useEffect(()=>{setTarget(null);setCollapsed(new Set());},[state.accountId,state.draft?.id]);
  const {
    draft,
    requestId,
    setRequestId,
    view,
    setView,
    filter,
    setFilter,
    sidebar,
    setSidebar,
    setResponse,
    setRequestError,
    setGuard,
    setRunCollection,
    updateData,
    openModal,
    addRequest,
  } = state;
  return (
    <aside
      className={`collection-sidebar ${sidebar ? "open" : ""}`}
      aria-label={t("集合目录")}
      id="collection-sidebar"
    >
      <div className="sidebar-heading">
        <Text weight="medium" size="2"> {t("集合")} </Text>
        <ToolButton
          label={t("新建集合")}
          disabled={!draft}
          onClick={() => openModal("new-collection")}
        >
          <Plus size={17} />
        </ToolButton>
      </div>
      <TextField.Root
        className="collection-search"
        placeholder={t("筛选请求…")}
        aria-label={t("筛选请求")}
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
      >
        <TextField.Slot>
          <Search size={14} />
        </TextField.Slot>
      </TextField.Root>
      <ScrollArea className="collection-tree">
        {collectionRows(draft?.data.collections ?? [],filter ? new Set() : collapsed).map(({collection,depth}) => (
          <div key={collection.id} className="collection-group" style={{marginInlineStart:Math.min(depth,6)*12}}>
            <Flex align="center" gap="2" className="collection-name">
              <ToolButton label={collapsed.has(collection.id)?t("展开目录"):t("折叠目录")} onClick={()=>setCollapsed(current=>{const next=new Set(current);if(next.has(collection.id))next.delete(collection.id);else next.add(collection.id);return next;})}>{collapsed.has(collection.id)?<ChevronRight size={13}/>:<ChevronDown size={13}/>}</ToolButton>
              <Folder size={15} />
              <Text size="2" className="truncate">
                {collection.name}
              </Text>
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  <Button
                    data-collection-actions={collection.id}
                    aria-label={t("{{value0}} 操作", { value0: collection.name })}
                    variant="ghost"
                    color="gray"
                    size="1"
                  >
                    <MoreHorizontal size={15} />
                  </Button>
                </DropdownMenu.Trigger>
                <DropdownMenu.Content>
                  <DropdownMenu.Item onSelect={()=>setTarget({kind:"edit",id:collection.id})}>{t("集合与目录设置")}</DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={()=>{setCollapsed(current=>{const next=new Set(current);next.delete(collection.id);return next;});setTarget({kind:"create",parent_id:collection.id});}}>{t("新建目录")}</DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => addRequest(collection.id)}> {t("新建请求")} </DropdownMenu.Item>
                  <DropdownMenu.Item
                    onSelect={() => {
                      setView("runner");
                      setRunCollection(collection.id);
                    }}
                  > {t("运行集合")} </DropdownMenu.Item>
                  <DropdownMenu.Separator />
                  <DropdownMenu.Item
                    color="red"
                    onSelect={() =>
                      setGuard({
                        title: message("删除集合"),
                        description: message("删除「{{value0}}」及其所有子目录和请求。保存后生效。", { value0: collection.name }),
                        action: () =>
                          updateData(data => removeCollectionTree(data,collection.id)),
                      })
                    }
                  > {t("删除集合")} </DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            </Flex>
            {(filter || !collapsed.has(collection.id)) && collection.requests
              .filter((r) =>
                `${r.name} ${r.url}`
                  .toLowerCase()
                  .includes(filter.toLowerCase()),
              )
              .map((r) => (
                <div
                  className={`request-row ${r.id === requestId && view === "requests" ? "selected" : ""}`}
                  key={r.id}
                >
                  <button
                    onClick={() => {
                      setRequestId(r.id);
                      setView("requests");
                      setResponse();
                      setRequestError();
                      setSidebar(false);
                    }}
                  >
                    <span
                      className={`method method-${requestLabel(r).toLowerCase()}`}
                    >
                      {requestLabel(r)}
                    </span>
                    <span className="truncate">{r.name || t("未命名请求")}</span>
                  </button>
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                      <Button
                        size="1"
                        variant="ghost"
                        color="gray"
                        aria-label={t("{{value0}} 请求操作", { value0: r.name })}
                      >
                        <MoreHorizontal size={13} />
                      </Button>
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content>
                      <DropdownMenu.Item
                        onSelect={() => {
                          const copy = {
                            ...structuredClone(r),
                            id: id(),
                            name: t("{{value0}} 副本", { value0: r.name }),
                          };
                          updateData((data) => ({
                            ...data,
                            collections: data.collections.map((c) =>
                              c.id === collection.id
                                ? { ...c, requests: [...c.requests, copy] }
                                : c,
                            ),
                          }));
                          setRequestId(copy.id);
                        }}
                      > {t("复制请求")} </DropdownMenu.Item>
                      <DropdownMenu.Item
                        color="red"
                        onSelect={() =>
                          setGuard({
                            title: message("删除请求"),
                            description: message("删除「{{value0}}」，保存后生效。", { value0: r.name }),
                            action: () => {
                              updateData((data) => ({
                                ...data,
                                collections: data.collections.map((c) => ({
                                  ...c,
                                  requests: c.requests.filter(
                                    (x) => x.id !== r.id,
                                  ),
                                })),
                              }));
                              if (requestId === r.id) setRequestId("");
                            },
                          })
                        }
                      > {t("删除请求")} </DropdownMenu.Item>
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                </div>
              ))}
            {(filter || !collapsed.has(collection.id)) && <Button
              size="1"
              color="gray"
              variant="ghost"
              className="add-request"
              onClick={() => addRequest(collection.id)}
            >
              <Plus size={13} /> {t("新建请求")} </Button>}
          </div>
        ))}
      </ScrollArea>
      {draft && target && (target.kind==="create" || draft.data.collections.some(c=>c.id===target.id)) && <CollectionSettingsDialog key={`${state.accountId}/${draft.id}/${target.kind}/${target.kind==="edit"?target.id:target.parent_id}`} target={target} close={()=>setTarget(null)}/>}
      <div className="sidebar-bottom">
        <Text size="1" color="gray">
          {t("{{count}} 个请求", { count: draft?.data.collections.reduce((n, c) => n + c.requests.length, 0) || 0 })} </Text>
        <Button
          size="1"
          color="gray"
          variant="ghost"
          onClick={() => openModal("import")}
        >
          <Upload size={13} /> {t("导入")} </Button>
      </div>
    </aside>
  );
}
