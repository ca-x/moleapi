import { useRef, useState } from "react";
import { Button, Callout, Flex, Text, TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { DataConfig } from "../data/types";
import { DataSqlEditor } from "../data/DataSqlEditor";
import { DataResults } from "../data/DataResults";
import { formatSql, selectedStatement } from "../data/model";
import { useDataEvents } from "../data/useDataEvents";
import SharedResultDatasetDialog, { type SaveSharedResultDataset } from "./SharedResultDatasetDialog";
import type { Result } from "../data/types";
import type { ProtocolEvent, SendMessage } from "../protocols/types";
export default function SharedDataSessionControls({ sessionId, config, events, dropped, open, sending, dark, send, onSaveResultDataset }: {
  sessionId: string; config: DataConfig; events: ProtocolEvent[]; dropped: number; open: boolean; sending: boolean; dark: boolean;
  onSaveResultDataset: SaveSharedResultDataset;
  send: (message: SendMessage) => Promise<boolean>;
}) {
  useLanguage();
  const model = useDataEvents(sessionId, events, dropped);
  const [sql, setSql] = useState(config.sql), [search, setSearch] = useState(""), [formatError, setFormatError] = useState(false);
  const [datasetResult, setDatasetResult] = useState<Result | null>(null);
  const selection = useRef({ from: 0, to: 0 });
  async function run() {
    if (!open || sending || model.active) return;
    const statement = selectedStatement(sql, config.source, selection.current.from, selection.current.to);
    if (!statement.trim()) return;
    const queryId = crypto.randomUUID();
    if (!model.reserve(queryId)) return;
    if (!await send({ kind: "data_query", query_id: queryId, sql: statement, read_only: config.read_only || config.source === "local_file" || config.source === "remote_file" })) model.release(queryId);
  }
  return <section aria-label={t("共享数据查询")}><Flex direction="column" gap="3">
    <Text size="1" color="gray">{t("运行光标所在的语句；有选区时仅运行选区。连接环境修改后重连生效。")}</Text>
    <Flex gap="2" wrap="wrap"><Button disabled={!open || sending || !!model.active} onClick={() => void run()}>{t("运行当前查询")}</Button><Button variant="soft" disabled={!open || !model.active || sending} onClick={() => model.active && void send({ kind: "data_cancel", query_id: model.active })}>{t("取消查询")}</Button><Button variant="ghost" onClick={() => { try { setSql(formatSql(sql, config.source)); setFormatError(false); } catch { setFormatError(true); } }}>{t("格式化 SQL")}</Button><Button variant="soft" disabled={!open || sending || !!model.active} onClick={() => void send({ kind: "data_schema_refresh" })}>{t("刷新数据结构")}</Button></Flex>
    <DataSqlEditor value={sql} change={setSql} source={config.source} tables={model.tables} dark={dark} selection={(from, to) => { selection.current = { from, to }; }} run={() => void run()} />
    <details><summary>{t("数据结构")}</summary><TextField.Root aria-label={t("搜索表和列")} value={search} onChange={(event) => setSearch(event.target.value)} />{model.schemaTruncated && <Text size="1">{t("仅显示部分结构，查询仍可直接使用完整表名。")}</Text>}{model.tables.filter((table) => !search || `${table.schema}.${table.name} ${table.columns.map((column) => column.name).join(" ")}`.toLowerCase().includes(search.toLowerCase())).map((table, index) => <details key={`${table.schema}/${table.name}/${index}`}><summary>{table.schema ? `${table.schema}.` : ""}{table.name}</summary><Button variant="ghost" onClick={() => setSql(`${sql.trim()}\nSELECT * FROM ${table.reference} LIMIT 100;`)}>{t("添加表查询")}</Button>{table.columns.map((column, index) => <Text as="div" size="2" key={index}>{column.name} · {column.data_type}</Text>)}</details>)}</details>
    {(model.error || model.processingError || formatError) && <Callout.Root color="red" role="alert"><Callout.Text>{model.error || model.processingError || t("SQL 语法结构有误，请检查括号或引号。")}</Callout.Text></Callout.Root>}
    {model.preparing && <Text role="status">{t("正在运行查询前脚本…")}</Text>}
    {model.preparation && <Flex direction="column" gap="1"><Text size="2">{t("查询前脚本已完成")}{model.preparation.sql_changed && <> · {t("本次 SQL 已由脚本修改")}</>}</Text>{model.preparation.tests.map((test) => <Text size="2" key={test.id}>{test.passed ? "✓" : "✕"} {test.name}: {test.actual}</Text>)}{model.preparation.logs.map((log, index) => <Text size="2" key={index}>{log.level}: {log.message}</Text>)}</Flex>}
    <DataResults result={model.result} onSaveDataset={() => setDatasetResult(model.result)} />
    {datasetResult && datasetResult === model.result && <SharedResultDatasetDialog key={datasetResult.id} result={datasetResult} onClose={() => setDatasetResult(null)} onSave={onSaveResultDataset} />}
    {model.processing && <Flex direction="column" gap="1"><Text size="2">{t("查询后处理已完成")}</Text>{model.processing.tests.map((test) => <Text size="2" key={test.id}>{test.passed ? "✓" : "✕"} {test.name}: {test.actual}</Text>)}{model.processing.logs.map((log, index) => <Text size="2" key={index}>{log.level}: {log.message}</Text>)}</Flex>}
  </Flex></section>;
}
