import { t, useLanguage } from "../../shared/i18n";
import { Button, Callout, Flex, Heading, Text } from "@radix-ui/themes";
import { ChevronRight, Plus, Upload, Workflow } from "lucide-react";
import { Choice } from "../../shared/ui";
import { safeMessage } from "../../shared/model";
import RequestEditor from "../requests/RequestEditor";
import EnvironmentsPage from "../environments/EnvironmentsPage";
import HistoryPage from "../history/HistoryPage";
import TestingPage from "../testing/TestingPage";
import SpecificationsPage from "../specifications/SpecificationsPage";
import { useWorkbench } from "./context";
import WebhooksPage from "../webhooks/WebhooksPage";
import { nav } from "./navigation";
export default function WorkbenchMain() {
  useLanguage();
  const {
    accountId,
    workspaces,
    draft,
    view,
    setView,
    request,
    updateRequest,
    send,
    save,
    dirty,
    saving,
    busy,
    sending,
    dark,
    response,
    requestError,
    protocolSession,
    addRequest,
    openModal,
    updateData,
  } = useWorkbench();
  return (
    <main className="main-workspace">
      {workspaces.error && (
        <Callout.Root color="red">
          <Callout.Text>{safeMessage(workspaces.error)}</Callout.Text>
        </Callout.Root>
      )}
      {!draft ? (
        <div className="workspace-empty">
          <img src="/logo.png" width="112" height="112" alt="MoleAPI" />
          <Heading size="6">
            {workspaces.isPending ? t("正在加载工作区…") : t("从一个工作区开始")}
          </Heading>
          <Text color="gray">{t("创建集合、调试 API，或导入已有的接口定义。")}</Text>
          <Flex gap="3">
            <Button onClick={() => openModal("new-workspace")}>
              <Plus size={16} /> {t("新建工作区")} </Button>
            <Button variant="soft" onClick={() => openModal("import")}>
              <Upload size={16} /> {t("导入")} </Button>
          </Flex>
        </div>
      ) : (
        <>
          <div className="workspace-bar">
            <Flex align="center" gap="2">
              <Text size="1" color="gray">
                {draft.name}
              </Text>
              <ChevronRight size={13} />
              <Text size="1">{nav.find((x) => x.id === view)?.label}</Text>
            </Flex>
            <Flex align="center" gap="3">
              <Choice
                value={draft.data.active_environment_id || "none"}
                onChange={(value) =>
                  updateData((data) => ({
                    ...data,
                    active_environment_id: value === "none" ? null : value,
                  }))
                }
                options={[
                  { value: "none", label: t("无环境") },
                  ...draft.data.environments.map((e) => ({
                    value: e.id,
                    label: e.name,
                  })),
                ]}
                label={t("当前环境")}
              />
              <Button
                size="1"
                variant="ghost"
                color="gray"
                onClick={() => setView("environments")}
              > {t("管理")} </Button>
            </Flex>
          </div>
          {view === "requests" &&
            (request ? (
              <RequestEditor
                key={`${draft.id}/${request.id}`}
                request={request}
                bodyScope={JSON.stringify([accountId,draft.id,draft.data.active_environment_id])}
                protocolConnected={
                  !!protocolSession.session &&
                  ["connecting", "open"].includes(protocolSession.session.state)
                }
                update={updateRequest}
                send={() => void send()}
                save={() => void save()}
                dirty={dirty}
                saving={saving}
                busy={busy}
                sending={sending}
                dark={dark}
                response={response}
                error={requestError}
              />
            ) : (
              <div className="workspace-empty">
                <Workflow size={35} />
                <Heading size="4">{t("选择一个请求")}</Heading>
                <Text color="gray">{t("从左侧集合选择请求，或创建一个新请求。")}</Text>
                <Button
                  onClick={() =>
                    draft.data.collections.length
                      ? addRequest(draft.data.collections[0].id)
                      : openModal("new-collection")
                  }
                >
                  <Plus size={16} /> {t("新建请求")} </Button>
              </div>
            ))}
          {view === "environments" && <EnvironmentsPage />}
          {view === "history" && <HistoryPage />}
          {view === "runner" && <TestingPage />}
          {view === "webhooks" && <WebhooksPage />}
          {view === "specifications" && <SpecificationsPage />}
        </>
      )}
    </main>
  );
}
