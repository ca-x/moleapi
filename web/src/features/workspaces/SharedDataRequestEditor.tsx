import { Flex, TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { Editor, Field } from "../../shared/ui";
import type { RequestSpec } from "../../shared/types";
import { DataSourceFields } from "../data/DataSourceFields";
import { DataSqlEditor } from "../data/DataSqlEditor";
import RequestAuthEditor from "../authentication/RequestAuthEditor";
import RequestScripts from "../scripts/RequestScripts";
import AssertionsEditor from "../testing/AssertionsEditor";
import ExtractionsEditor from "../testing/ExtractionsEditor";
export default function SharedDataRequestEditor({ request, dark, isCurrent, onChange }: {
  request: RequestSpec; dark: boolean; isCurrent: () => boolean; onChange: (patch: Partial<RequestSpec>) => void;
}) {
  useLanguage();
  const config = request.protocol?.kind === "data" ? request.protocol : null;
  if (!config) return null;
  const change = (patch: Partial<typeof config>) => { if (isCurrent()) onChange({ protocol: { ...config, ...patch } }); };
  return <section aria-label={t("Data 请求设置")}><Flex direction="column" gap="3">
    <DataSourceFields config={config} change={change} connected={false} isCurrent={isCurrent} />
    {config.source !== "local_file" && <Field label={t("数据源地址")}><TextField.Root value={request.url} onChange={(event) => onChange({ url: event.target.value })} /></Field>}
    <DataSqlEditor value={config.sql} change={(sql) => change({ sql })} source={config.source} tables={[]} dark={dark} selection={() => {}} run={() => {}} />
    <Editor label={t("查询前 JavaScript 脚本")} value={config.query_pre_script} onChange={(query_pre_script) => change({ query_pre_script })} dark={dark} language="javascript" height="160px" />
    <details><summary>{t("鉴权")}</summary><RequestAuthEditor auth={request.auth} change={(auth) => onChange({ auth })} dark={dark} tokenManagement={false} /></details>
    <details><summary>{t("断言")}</summary><AssertionsEditor request={request} update={onChange} dark={dark} /><ExtractionsEditor request={request} update={onChange} /></details>
    <details><summary>{t("脚本")}</summary><RequestScripts request={request} update={onChange} dark={dark} /></details>
  </Flex></section>;
}
