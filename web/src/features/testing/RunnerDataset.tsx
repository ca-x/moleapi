import {useEffect,useRef,useState} from "react";
import {Button,Callout,Flex,Table,Text,TextField} from "@radix-ui/themes";
import {Choice,Editor,Field} from "../../shared/ui";
import {api,pickFile} from "../../shared/api";
import {t} from "../../shared/i18n";
import type {RunOptions} from "../../shared/types";
export interface RunnerConfig {format:"csv"|"json";source:string;iterations:string}
export function runnerOptions(config:RunnerConfig):RunOptions {
 const iterations=config.iterations.trim()?Number(config.iterations):undefined;
 if(iterations!==undefined&&(!Number.isInteger(iterations)||iterations<1||iterations>100))throw new Error("iterations");
 if(new TextEncoder().encode(config.source).length>1024*1024)throw new Error("dataset limit");
 return {...(iterations===undefined?{}:{iterations}),...(config.source.trim()?{dataset:{format:config.format,source:config.source}}:{})};
}
interface Preview {columns:string[];rows:Record<string,unknown>[]}
export default function RunnerDataset({workspaceId,scope,value,disabled,dark,onChange,onBusyChange,sourceReadOnly=false}:{workspaceId:string;scope:string;value:RunnerConfig;disabled:boolean;dark:boolean;onChange:(value:RunnerConfig)=>void;onBusyChange?:(busy:boolean)=>void;sourceReadOnly?:boolean}){
 const [preview,setPreview]=useState<{identity:string;value:Preview}|null>(null),[pending,setPending]=useState(false),[error,setError]=useState(false);
 const identity=JSON.stringify([scope,value]),latest=useRef(identity);latest.current=identity;const mounted=useRef(true),sequence=useRef(0);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;sequence.current++;onBusyChange?.(false);};},[]);
 const valid=(origin:string,ticket:number)=>mounted.current&&latest.current===origin&&sequence.current===ticket;
 async function read(){const origin=identity,ticket=++sequence.current;setPending(true);onBusyChange?.(true);setError(false);try{const source=await pickFile({extensions:["csv","json","txt"],maximum:1024*1024,label:t("测试数据集")});if(source!==null&&valid(origin,ticket))onChange({...value,source});}catch{if(valid(origin,ticket))setError(true);}finally{if(mounted.current&&sequence.current===ticket){setPending(false);onBusyChange?.(false);}}}
 async function inspect(){const origin=identity,ticket=++sequence.current;setPending(true);onBusyChange?.(true);setError(false);try{const options=runnerOptions(value);if(!options.dataset)return;const result=await api<Preview>("/api/testing/dataset/preview","POST",{workspace_id:workspaceId,dataset:options.dataset});if(valid(origin,ticket))setPreview({identity:origin,value:result});}catch{if(valid(origin,ticket))setError(true);}finally{if(mounted.current&&sequence.current===ticket){setPending(false);onBusyChange?.(false);}}}
 const result=preview?.identity===identity?preview.value:null;
 return <Flex direction="column" gap="3">
  <Flex gap="3" wrap="wrap"><Field label={t("迭代次数")}><TextField.Root type="number" min="1" max="100" placeholder={t("自动：数据行数或 1")} disabled={disabled} value={value.iterations} onChange={event=>onChange({...value,iterations:event.target.value})}/></Field><Field label={t("数据格式")}><Choice label={t("数据格式")} value={value.format} disabled={disabled||pending||sourceReadOnly} options={[{value:"json",label:"JSON"},{value:"csv",label:"CSV"}]} onChange={format=>onChange({...value,format})}/></Field><Button variant="soft" disabled={disabled||pending||sourceReadOnly} onClick={()=>void read()}>{t("导入测试数据")}</Button><Button variant="soft" disabled={disabled||pending||!value.source.trim()} onClick={()=>void inspect()}>{t("预览数据行")}</Button><Button variant="ghost" disabled={disabled||pending||sourceReadOnly||!value.source} onClick={()=>onChange({...value,source:""})}>{t("清除测试数据")}</Button></Flex>
  <Text size="1" color="gray">{t("CSV 使用表头，JSON 使用对象数组；数据仅用于本次运行，可通过变量占位符和 pm.iterationData 读取。最多 100 行、1 MiB。")}</Text>
  <Editor value={value.source} onChange={source=>onChange({...value,source})} readOnly={disabled||pending||sourceReadOnly} dark={dark} jsonMode={value.format==="json"} height="180px" label={t("测试数据源")}/>
  {pending&&<Text role="status">{t("正在读取测试数据…")}</Text>}{error&&<Callout.Root color="red"><Callout.Text role="alert">{t("测试数据无效，请检查格式、迭代次数和大小限制。")}</Callout.Text></Callout.Root>}
  {result&&<><Text size="2">{t("数据集包含 {{rows}} 行、{{columns}} 列。",{rows:result.rows.length,columns:result.columns.length})}</Text><div style={{overflowX:"auto",maxWidth:"100%"}}><Table.Root><Table.Header><Table.Row>{result.columns.map(column=><Table.ColumnHeaderCell key={column}>{column}</Table.ColumnHeaderCell>)}</Table.Row></Table.Header><Table.Body>{result.rows.slice(0,10).map((row,index)=><Table.Row key={index}>{result.columns.map(column=><Table.Cell key={column} style={{maxWidth:200,overflowWrap:"anywhere"}}>{String(typeof row[column]==="object"?JSON.stringify(row[column]):row[column]??"").slice(0,200)}</Table.Cell>)}</Table.Row>)}</Table.Body></Table.Root></div></>}
 </Flex>;
}
