import { t, useLanguage, message } from "../../shared/i18n";
import {
  Badge,
  Button,
  Callout,
  Flex,
  Heading,
  Table,
  Text,
} from "@radix-ui/themes";
import { api } from "../../shared/api";
import { bytes, safeMessage } from "../../shared/model";
import { ResponsePane } from "../requests/ResponsePane";
import { useWorkbench } from "../workbench/context";

export default function HistoryPage() {
  useLanguage();
  const state = useWorkbench();
  const {
    dark,
    draft,
    setGuard,
    historyResponse,
    setHistoryResponse,
    history,
  } = state;
  if (!draft) return null;
  return (
    <div className="page-panel">
      <div className="page-heading">
        <div>
          <Heading size="5">{t("请求历史")}</Heading>
          <Text size="2" color="gray"> {t("最近的执行记录，包含真实状态、耗时与响应。历史会保存接口返回的响应内容，可能包含敏感信息，可随时清空。")} </Text>
        </div>
        <Button
          color="gray"
          variant="soft"
          onClick={() =>
            setGuard({
              title: message("清空历史"),
              description: message("清空此工作区的请求历史，不影响已保存请求。"),
              action: async () => {
                await api(`/api/workspaces/${draft.id}/history`, "DELETE");
                setHistoryResponse(null);
                await history.refetch();
              },
            })
          }
        > {t("清空历史")} </Button>
      </div>
      {history.error && (
        <Callout.Root color="red">
          <Callout.Text>{safeMessage(history.error)}</Callout.Text>
        </Callout.Root>
      )}
      <Table.Root>
        <Table.Header>
          <Table.Row>
            <Table.ColumnHeaderCell>{t("请求")}</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell>{t("状态")}</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell>{t("耗时")}</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell>{t("时间")}</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell />
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {history.data?.map((entry) => (
            <Table.Row key={entry.id}>
              <Table.Cell>
                <Flex gap="2">
                  <span
                    className={`method method-${entry.method.toLowerCase()}`}
                  >
                    {entry.method}
                  </span>
                  <Text>{entry.request_name}</Text>
                </Flex>
              </Table.Cell>
              <Table.Cell>
                <Badge color={entry.status < 400 ? "green" : "red"}>
                  {entry.status}
                </Badge>
              </Table.Cell>
              <Table.Cell className="mono">
                {entry.elapsed_ms} ms · {bytes(entry.size_bytes)}
              </Table.Cell>
              <Table.Cell>
                {new Date(entry.created_at).toLocaleString()}
              </Table.Cell>
              <Table.Cell>
                <Button
                  size="1"
                  color="gray"
                  variant="ghost"
                  onClick={() => setHistoryResponse(entry.response)}
                > {t("查看响应")} </Button>
              </Table.Cell>
            </Table.Row>
          ))}
        </Table.Body>
      </Table.Root>
      {history.data?.length === 0 && (
        <Text color="gray">{t("还没有请求记录。发送一个请求后即可查看。")}</Text>
      )}
      {historyResponse && (
        <ResponsePane
          response={historyResponse}
          error=""
          dark={dark}
          busy={false}
        />
      )}
    </div>
  );
}
