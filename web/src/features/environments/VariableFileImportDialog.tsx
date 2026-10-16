import {useEffect,useLayoutEffect,useRef,useState} from "react";
import {Button,Callout,Dialog,Flex,Text,TextArea,TextField} from "@radix-ui/themes";
import {api,pickFile} from "../../shared/api";
import {errorCopy,type ErrorCopy} from "../../shared/i18n/errors";
import {t,translateCopy,useLanguage} from "../../shared/i18n";
import {id} from "../../shared/model";
import type {VariableFileImport} from "../../shared/types";
import {useWorkbench} from "../workbench/context";

export default function VariableFileImportDialog({open,onOpenChange}:{open:boolean;onOpenChange:(open:boolean)=>void}){
  useLanguage();
  const state=useWorkbench();
  const [content,setContent]=useState(""),[name,setName]=useState(""),[preview,setPreview]=useState<VariableFileImport|null>(null);
  const [pending,setPending]=useState(false),[problem,setProblem]=useState<ErrorCopy>("");
  const mounted=useRef(true),latest=useRef(""),scopeEpoch=useRef(0),previewOrigin=useRef(""),attemptedId=useRef<string|null>(null);
  const identity=JSON.stringify([state.accountId,state.draft?.id,state.draft?.revision]);latest.current=identity;
  const previous=useRef({account:state.accountId,workspace:state.draft?.id});
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  useLayoutEffect(()=>{
    const prior=previous.current;previous.current={account:state.accountId,workspace:state.draft?.id};
    if(prior.account!==state.accountId){scopeEpoch.current++;setContent("");setName("");setPreview(null);setProblem("");attemptedId.current=null;previewOrigin.current="";}
    else if(prior.workspace!==state.draft?.id){scopeEpoch.current++;setPreview(null);setProblem(t("工作区已切换，请重新预览变量文件。"));attemptedId.current=null;previewOrigin.current="";}
  },[state.accountId,state.draft?.id]);
  function change(next:boolean){onOpenChange(next);if(!next){scopeEpoch.current++;setContent("");setName("");setPreview(null);setProblem("");attemptedId.current=null;previewOrigin.current="";}}
  async function choose(){if(pending)return;const origin=identity,epoch=scopeEpoch.current;setPending(true);setProblem("");try{
    const text=await pickFile({extensions:["json"],maximum:1024*1024,label:t("Postman 变量文件")});
    if(text!==null&&mounted.current&&latest.current===origin&&scopeEpoch.current===epoch){setContent(text);setPreview(null);attemptedId.current=null;previewOrigin.current="";}
  }catch(error){if(mounted.current&&latest.current===origin&&scopeEpoch.current===epoch)setProblem(errorCopy(error));}
  finally{if(mounted.current)setPending(false);}}
  async function inspect(){if(pending||!state.draft||!content)return;const origin=identity,epoch=scopeEpoch.current;setPending(true);setProblem("");try{
    const result=await api<VariableFileImport>(`/api/workspaces/${state.draft.id}/variables/import-preview`,"POST",{content,expected_revision:state.draft.revision});
    if(mounted.current&&latest.current===origin&&scopeEpoch.current===epoch){setPreview(result);setName(result.name);previewOrigin.current=origin;}
  }catch(error){if(mounted.current&&latest.current===origin&&scopeEpoch.current===epoch)setProblem(errorCopy(error));}
  finally{if(mounted.current)setPending(false);}}
  async function confirm(){if(pending||state.saving||!state.draft||!preview)return;
    if(previewOrigin.current!==identity){setProblem(t("工作区已变化，请重新预览变量文件。"));setPreview(null);return;}
    const origin=identity,epoch=scopeEpoch.current,current=()=>mounted.current&&latest.current===origin&&scopeEpoch.current===epoch;
    const title=name.trim();let identifier="";
    if(preview.scope==="environment"){
      if(!title||new TextEncoder().encode(title).length>128){setProblem(t("请输入 1–128 字节的环境名称。"));return;}
      identifier=attemptedId.current??id();
      if(state.draft.data.environments.some(item=>item.name===title&&item.id!==identifier)){
        setProblem(t("环境名称已存在，请改名后重试。"));return;
      }
      attemptedId.current=identifier;
    }
    setPending(true);setProblem("");
    try{
      if(preview.scope==="environment"){
        state.updateData(data=>({...data,environments:[...data.environments.filter(item=>item.id!==identifier),{id:identifier,name:title,variables:preview.variables}],active_environment_id:identifier}));
      }else state.updateData(data=>({...data,global_variables:preview.variables}));
      const saved=await state.save(true);
      if(current()){
        const written=preview.scope==="environment"
          ?saved?.data.environments.some(item=>item.id===identifier&&item.name===title&&JSON.stringify(item.variables)===JSON.stringify(preview.variables))
          :JSON.stringify(saved?.data.global_variables)===JSON.stringify(preview.variables);
        if(written)change(false);
        else setProblem(t("变量文件未写入工作区，请检查保存冲突后重试。"));
      }
    }catch(error){if(current())setProblem(errorCopy(error));}
    finally{if(mounted.current)setPending(false);}
  }
  return <Dialog.Root open={open} onOpenChange={change}><Dialog.Content maxWidth="min(560px, calc(100vw - 24px))">
    <Dialog.Title>{t("导入 Postman 变量文件")}</Dialog.Title>
    <Dialog.Description>{t("先预览文件，再新建环境或明确替换项目全局变量。")}</Dialog.Description>
    <Flex direction="column" gap="3" mt="3">
      <Button variant="soft" disabled={pending} onClick={()=>void choose()}>{t("选择 JSON 文件")}</Button>
      <TextArea aria-label={t("Postman 变量文件内容")} value={content} rows={8} placeholder={t("粘贴 Postman 环境或全局变量 JSON")}
        onChange={event=>{setContent(event.target.value);setPreview(null);attemptedId.current=null;previewOrigin.current="";}}/>
      <Button variant="soft" disabled={pending||!content.trim()} onClick={()=>void inspect()}>{t("预览变量文件")}</Button>
      {preview&&<>
        <Text size="2">{preview.scope==="environment"?t("新环境"):t("项目全局变量")} · {t("{{count}} 个变量",{count:preview.variables.length})}</Text>
        {preview.scope==="environment"?<TextField.Root aria-label={t("导入环境名称")} value={name} maxLength={128} onChange={event=>setName(event.target.value)}/>:
          <Text size="2" color="amber">{t("将替换当前 {{old}} 个项目全局变量，导入 {{next}} 个。",{old:state.draft?.data.global_variables?.length??0,next:preview.variables.length})}</Text>}
        {preview.warnings.map((warning,index)=><Text key={index} size="2" color="amber">{warning==="postman_variable_row_metadata_omitted"?t("部分 Postman 变量元数据未导入。"):warning}</Text>)}
      </>}
      {problem&&<Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(problem)}</Callout.Text></Callout.Root>}
      <Flex justify="end" gap="2"><Button variant="soft" color="gray" disabled={pending} onClick={()=>change(false)}>{t("取消")}</Button>
        <Button disabled={!preview||pending||state.saving} loading={pending} onClick={()=>void confirm()}>{preview?.scope==="globals"?t("替换项目全局变量"):t("新建导入环境")}</Button></Flex>
    </Flex>
  </Dialog.Content></Dialog.Root>;
}
