import {useEffect,useRef,useState} from "react";
import {Button,Callout,Dialog,Flex,Text,TextField} from "@radix-ui/themes";
import {api} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";
import {id} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import {resultDatasetSource} from "./dataset";
import {resultDatasetMessage as message} from "./resultDatasetMessage";
import type {Result} from "./types";
import type {SavedDataset} from "../../shared/types";


export default function DataResultDatasetDialog({open,onOpenChange,result,scope}:{open:boolean;onOpenChange:(open:boolean)=>void;result:Result|null;scope:string}){
  useLanguage();
  const state=useWorkbench(),[name,setName]=useState(""),[pending,setPending]=useState(false),[problem,setProblem]=useState("");
  const attemptedId=useRef<string|null>(null),mounted=useRef(true);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  useEffect(()=>{
    if(attemptedId.current)setProblem(t("工作区或查询已变化，请确认目标后重试保存。"));
    else {setName("");setProblem("");}
    attemptedId.current=null;
  },[scope]);
  const identity=JSON.stringify([scope,state.accountId,state.draft?.id,state.request?.id,result?.id,open]);
  const latest=useRef(identity);latest.current=identity;
  function change(next:boolean){onOpenChange(next);if(!next){setProblem("");setName("");attemptedId.current=null;}}
  async function save(){if(pending||!state.draft||!result)return;
    if(state.saving){setProblem(t("工作区正在保存，请完成后重试数据集保存。"));return;}
    const origin=identity,workspace=state.draft.id;const current=()=>mounted.current&&latest.current===origin;
    setPending(true);setProblem("");
    try{
      const title=name.trim();if(!title||new TextEncoder().encode(title).length>256){setProblem(t("请输入 1–256 字节的数据集名称。"));return;}
      const source=resultDatasetSource(result),existing=state.draft.data.datasets??[];
      if(existing.length>=20&&!attemptedId.current){setProblem(t("工作区最多保存 20 个数据集。"));return;}
      const identifier=attemptedId.current??id();attemptedId.current=identifier;
      const total=existing.filter(dataset=>dataset.id!==identifier).reduce((size,dataset)=>size+(dataset.source?new TextEncoder().encode(dataset.source.source).length:0),new TextEncoder().encode(source.source).length);
      if(total>4*1024*1024){setProblem(t("工作区数据集源总量超过 4 MiB。"));return;}
      await api("/api/testing/dataset/preview","POST",{workspace_id:workspace,dataset:source});
      if(!current())return;
      if(state.saving){setProblem(t("工作区正在保存，请完成后重试数据集保存。"));return;}
      const dataset:SavedDataset={id:identifier,name:title,description:"",secret:true,source};
      state.updateData(data=>({...data,datasets:[...(data.datasets??[]).filter(item=>item.id!==identifier),dataset]}));
      const saved=await state.save(true);
      if(current()){
        if(saved?.data.datasets?.some(item=>item.id===identifier&&item.name===title&&item.source?.source===source.source))change(false);
        else setProblem(t("数据集未写入工作区，请检查保存冲突后重试。"));
      }
    }catch(error){if(current())setProblem(message(error));}
    finally{if(mounted.current)setPending(false);}
  }
  return <Dialog.Root open={open} onOpenChange={change}><Dialog.Content maxWidth="min(480px, calc(100vw - 24px))">
    <Dialog.Title>{t("保存查询结果为数据集")}</Dialog.Title>
    <Dialog.Description>{t("保存本次查询返回的全部行。整数与小数按字符串保存以保持精度；数据集默认为私密。")}</Dialog.Description>
    {result&&<Text size="2" color="gray">{t("{{rows}} 行 · {{columns}} 列",{rows:result.rows.length,columns:result.columns.length})}</Text>}
    <TextField.Root aria-label={t("数据集名称")} value={name} maxLength={256} placeholder={t("数据集名称")} onChange={event=>setName(event.target.value)}/>
    {problem&&<Callout.Root color="red" role="alert"><Callout.Text>{problem}</Callout.Text></Callout.Root>}
    <Flex justify="end" gap="2"><Button variant="soft" color="gray" disabled={pending} onClick={()=>change(false)}>{t("取消")}</Button><Button disabled={pending||!name.trim()} loading={pending} onClick={()=>void save()}>{t("保存数据集")}</Button></Flex>
  </Dialog.Content></Dialog.Root>;
}
