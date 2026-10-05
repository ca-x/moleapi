import {useState,useRef,useEffect} from "react";
import {Button,Checkbox,Flex,Text,TextField,TextArea,Callout} from "@radix-ui/themes";
import {Choice,Field} from "../../shared/ui";
import {t,useLanguage,translateCopy} from "../../shared/i18n";
import {errorCopy,type ErrorCopy} from "../../shared/i18n/errors";
import type {DataConfig} from "./types";
import {pickDataFile} from "./pickFile";
export function DataSourceFields({config,change,connected}:{config:DataConfig;change:(patch:Partial<DataConfig>)=>void;connected:boolean}) {
  useLanguage();const [pending,setPending]=useState(false),[error,setError]=useState<ErrorCopy>("");
  const live=useRef(true),latest=useRef({source:config.source,change,connected});latest.current={source:config.source,change,connected};useEffect(()=>{live.current=true;return()=>{live.current=false;};},[]);
  const file=config.source==="local_file"||config.source==="remote_file";
  async function pick(){setPending(true);setError("");try{const origin=config.source;const result=await pickDataFile();if(result&&live.current&&latest.current.source===origin&&!latest.current.connected){const suffix=result.name.split(".").at(-1)?.toLowerCase();latest.current.change({file_name:result.name,file_base64:result.base64,file_format:suffix==="parquet"?"parquet":suffix==="json"||suffix==="ndjson"||suffix==="jsonl"?"json":"csv"});}}catch(e){if(live.current)setError(errorCopy(e));}finally{if(live.current)setPending(false);}}
  return <div className="data-source-fields">
    <Flex gap="3" wrap="wrap" align="end">
      <Choice label={t("数据源类型")} value={config.source} disabled={connected||pending} options={[{value:"postgresql",label:"PostgreSQL"},{value:"mysql",label:"MySQL"},{value:"local_file",label:t("本地文件")},{value:"remote_file",label:t("远程文件")}]} onChange={source=>change({source,file_base64:"",read_only:source==="local_file"||source==="remote_file"?true:config.read_only})}/>
      {config.source==="local_file"&&<Button variant="soft" disabled={connected||pending} loading={pending} onClick={()=>void pick()}>{t("选择数据文件")}</Button>}
      {config.file_name&&config.source==="local_file"&&<Text size="2" className="wrap-anywhere">{config.file_name}</Text>}
      {file&&<><Choice label={t("文件格式")} value={config.file_format} disabled={connected} options={[{value:"csv",label:"CSV"},{value:"json",label:"JSON / JSONL"},{value:"parquet",label:"Parquet"}]} onChange={file_format=>change({file_format})}/><Field label={t("SQL 表名")}><TextField.Root value={config.table_name} disabled={connected} maxLength={128} onChange={e=>change({table_name:e.target.value})}/></Field></>}
      <Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={file||config.read_only} disabled={file} onCheckedChange={v=>change({read_only:v===true})}/>{t("只读查询")}</Flex></Text>
      {!file&&<Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={config.tls} disabled={connected} onCheckedChange={v=>change({tls:v===true})}/>{t("使用 TLS")}</Flex></Text>}
      {file&&config.file_format==="csv"&&<Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={config.csv_header} disabled={connected} onCheckedChange={v=>change({csv_header:v===true})}/>{t("首行为列名")}</Flex></Text>}
    </Flex>
    {!file&&<details><summary>{t("附加 CA 证书")}</summary><Field label={t("CA 证书（PEM）")}><TextArea value={config.ca_pem} disabled={connected} maxLength={65536} className="mono" onChange={e=>change({ca_pem:e.target.value})}/></Field></details>}
    <Text size="1" color="gray">{file?t("文件查询仅访问选定数据。CSV 小数保留为文本，可显式 CAST 为 DECIMAL。"):t("连接串使用 postgresql:// 或 mysql://；用户名与密码也可在鉴权页填写。取消后连接会关闭，写入结果可能未知。")}</Text>
    {error&&<Callout.Root color="red"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
  </div>;
}
