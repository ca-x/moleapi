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
  const state = useWorkbench();
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
      aria-label="集合目录"
      id="collection-sidebar"
    >
      <div className="sidebar-heading">
        <Text weight="medium" size="2">
          集合
        </Text>
        <ToolButton
          label="新建集合"
          disabled={!draft}
          onClick={() => openModal("new-collection")}
        >
          <Plus size={17} />
        </ToolButton>
      </div>
      <TextField.Root
        className="collection-search"
        placeholder="筛选请求…"
        aria-label="筛选请求"
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
      >
        <TextField.Slot>
          <Search size={14} />
        </TextField.Slot>
      </TextField.Root>
      <ScrollArea className="collection-tree">
        {draft?.data.collections.map((collection) => (
          <div key={collection.id} className="collection-group">
            <Flex align="center" gap="2" className="collection-name">
              <ChevronDown size={13} />
              <Folder size={15} />
              <Text size="2" className="truncate">
                {collection.name}
              </Text>
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  <Button
                    aria-label={`${collection.name} 操作`}
                    variant="ghost"
                    color="gray"
                    size="1"
                  >
                    <MoreHorizontal size={15} />
                  </Button>
                </DropdownMenu.Trigger>
                <DropdownMenu.Content>
                  <DropdownMenu.Item onSelect={() => addRequest(collection.id)}>
                    新建请求
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    onSelect={() => {
                      setView("runner");
                      setRunCollection(collection.id);
                    }}
                  >
                    运行集合
                  </DropdownMenu.Item>
                  <DropdownMenu.Separator />
                  <DropdownMenu.Item
                    color="red"
                    onSelect={() =>
                      setGuard({
                        title: "删除集合",
                        description: `删除「${collection.name}」及其请求。保存后生效。`,
                        action: () =>
                          updateData((data) => ({
                            ...data,
                            collections: data.collections.filter(
                              (c) => c.id !== collection.id,
                            ),
                          })),
                      })
                    }
                  >
                    删除集合
                  </DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            </Flex>
            {collection.requests
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
                    <span className="truncate">{r.name || "未命名请求"}</span>
                  </button>
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                      <Button
                        size="1"
                        variant="ghost"
                        color="gray"
                        aria-label={`${r.name} 请求操作`}
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
                            name: `${r.name} 副本`,
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
                      >
                        复制请求
                      </DropdownMenu.Item>
                      <DropdownMenu.Item
                        color="red"
                        onSelect={() =>
                          setGuard({
                            title: "删除请求",
                            description: `删除「${r.name}」，保存后生效。`,
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
                      >
                        删除请求
                      </DropdownMenu.Item>
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                </div>
              ))}
            <Button
              size="1"
              color="gray"
              variant="ghost"
              className="add-request"
              onClick={() => addRequest(collection.id)}
            >
              <Plus size={13} />
              新建请求
            </Button>
          </div>
        ))}
      </ScrollArea>
      <div className="sidebar-bottom">
        <Text size="1" color="gray">
          {draft?.data.collections.reduce((n, c) => n + c.requests.length, 0) ||
            0}{" "}
          个请求
        </Text>
        <Button
          size="1"
          color="gray"
          variant="ghost"
          onClick={() => openModal("import")}
        >
          <Upload size={13} />
          导入
        </Button>
      </div>
    </aside>
  );
}
