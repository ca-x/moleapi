import {useState,useRef,useEffect} from "react";
import {Badge,Button,Callout,Flex,Heading,Tabs,Text,TextField} from "@radix-ui/themes";
import {Play,Square,RefreshCw,WandSparkles} from "lucide-react";
import {t,useLanguage,translateCopy} from "../../shared/i18n";
import {errorCopy,type ErrorCopy} from "../../shared/i18n/errors";
import {useWorkbench} from "../workbench/context";
import {DataSourceFields} from "./DataSourceFields";
import {DataSqlEditor} from "./DataSqlEditor";
import {DataResults} from "./DataResults";
import {useDataEvents} from "./useDataEvents";
import {selectedStatement,formatSql} from "./model";
import type {DataConfig} from "./types";
export default function DataWorkbench(){
  useLanguage();const state=useWorkbench(),channel=state.protocolSession;
  const config=state.request?.protocol?.kind==="data"?state.request.protocol:null;
  const model=useDataEvents(channel.session?.id,channel.events,channel.dropped);
  const [tab,setTab]=useState("query"),[search,setSearch]=useState(""),[error,setError]=useState<ErrorCopy>("");
  const selection=useRef({from:0,to:0});
  const identity=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id,channel.session?.id]);
  const scope=useRef(identity);scope.current=identity;
  const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const dispatch=useRef<()=>void>(()=>{});dispatch.current=()=>{if(open)void run();else void channel.connect();};
  useEffect(()=>{const invoke=()=>dispatch.current();state.dataRun.current=invoke;return()=>{if(state.dataRun.current===invoke)state.dataRun.current=null;};},[state.dataRun]);
  const open=channel.session?.state==="open",connected=channel.busy||open||channel.session?.state==="connecting";
  function change(patch:Partial<DataConfig>){const current=state.request?.protocol;if(current?.kind==="data")state.updateRequest({protocol:{...current,...patch}});}
  async function run(){if(!config||!open||model.active)return;setError("");const selected=selectedStatement(config.sql,config.source,selection.current.from,selection.current.to);if(!selected)return;
    const origin=identity;const id=crypto.randomUUID();if(!model.reserve(id))return;const accepted=await channel.send({kind:"data_query",query_id:id,sql:selected,read_only:config.source==="local_file"||config.source==="remote_file"||config.read_only});if(mounted.current&&scope.current===origin){if(accepted)setTab("view");else model.release(id);}}
  function format(){if(!config)return;try{change({sql:formatSql(config.sql,config.source)});setError("");}catch(e){setError(errorCopy(e));}}
  if(!config)return null;
  const problem=translateCopy(error)||model.error||channel.error;
  return <section className="data-workbench" aria-label={t("Data 客户端")}>
    <Flex className="data-toolbar" gap="3" align="center" justify="between" wrap="wrap"><Flex gap="3" align="center" wrap="wrap"><Heading size="3">{t("数据查询")}</Heading><Badge color={open?"green":"gray"}>{open?t("已连接"):channel.session?.state==="connecting"?t("连接中"):channel.session?.state==="error"?t("连接失败"):t("未连接")}</Badge>{model.info&&<Text size="1" className="mono wrap-anywhere">{model.info.public_url}{model.info.tls?model.info.tls_verified?" · TLS":` · ${t("TLS 未验证")}`:""}</Text>}</Flex><Flex gap="2"><Button variant="soft" color="gray" disabled={!connected} onClick={()=>void channel.close()}><Square size={14}/>{t("断开")}</Button></Flex></Flex>
    <DataSourceFields key={`${state.accountId}/${state.selectedId}/${state.request?.id}/${state.draft?.data.active_environment_id}`} config={config} change={change} connected={connected}/>
    {problem&&<Callout.Root color="red" role="alert"><Callout.Text>{problem}</Callout.Text></Callout.Root>}
    {channel.session?.reason&&<Text color="gray" size="1">{channel.session.reason}</Text>}
    <Tabs.Root value={tab} onValueChange={setTab} className="data-tabs"><Tabs.List><Tabs.Trigger value="query">{t("查询")}</Tabs.Trigger><Tabs.Trigger value="view">{t("数据视图")}</Tabs.Trigger></Tabs.List>
      <Tabs.Content value="query"><div className="data-query-layout"><aside className="data-schema" aria-label={t("数据结构浏览器")}><Flex gap="2" align="center" justify="between"><Text size="2" weight="medium">{t("数据结构")}</Text><Button size="1" variant="ghost" disabled={!open||!!model.active||channel.sending} aria-label={t("刷新数据结构")} onClick={()=>void channel.send({kind:"data_schema_refresh"})}><RefreshCw size={14}/></Button></Flex><TextField.Root value={search} onChange={e=>setSearch(e.target.value)} placeholder={t("搜索表和列")} aria-label={t("搜索表和列")}/>{model.schemaTruncated&&<Text size="1" color="gray">{t("仅显示部分结构，查询仍可直接使用完整表名。")}</Text>}{model.tables.filter(table=>!search||`${table.schema}.${table.name} ${table.columns.map(c=>c.name).join(" ")}`.toLowerCase().includes(search.toLowerCase())).map((table,index)=><details key={`${table.schema}/${table.name}/${index}`}><summary>{table.schema?`${table.schema}.`:""}{table.name}</summary><Button size="1" variant="ghost" onClick={()=>change({sql:`${config.sql.trim()}\nSELECT * FROM ${table.reference} LIMIT 100;`})}>{t("添加表查询")}</Button>{table.columns.map((column,index)=><button className="data-schema-column" key={`${column.name}/${index}`} onClick={()=>change({sql:config.sql+`\nSELECT ${column.reference} FROM ${table.reference} LIMIT 100;`})}><span>{column.name}</span><Text size="1" color="gray">{column.data_type}{column.nullable?" · NULL":""}</Text></button>)}</details>)}</aside>
        <div className="data-editor-panel"><Flex gap="3" align="center" wrap="wrap"><Button disabled={!open||!!model.active||channel.sending} onClick={()=>void run()}><Play size={14}/>{t("运行当前查询")}</Button><Button variant="soft" disabled={!model.active||channel.sending} onClick={()=>model.active&&void channel.send({kind:"data_cancel",query_id:model.active})}>{t("取消查询")}</Button><Button variant="ghost" color="gray" onClick={format}><WandSparkles size={14}/>{t("格式化 SQL")}</Button></Flex><Text size="1" color="gray">{t("运行光标所在的语句；有选区时仅运行选区。连接环境修改后重连生效。")}</Text><DataSqlEditor run={()=>dispatch.current()} value={config.sql} change={sql=>change({sql})} source={config.source} tables={model.tables} dark={state.dark} selection={(from,to)=>{selection.current={from,to};}}/></div></div></Tabs.Content>
      <Tabs.Content value="view"><Flex gap="3" align="center" wrap="wrap"><Button size="1" variant="soft" disabled={!model.active||channel.sending} onClick={()=>model.active&&void channel.send({kind:"data_cancel",query_id:model.active})}>{t("取消查询")}</Button></Flex><DataResults result={model.result}/></Tabs.Content>
    </Tabs.Root>
  </section>;
}
