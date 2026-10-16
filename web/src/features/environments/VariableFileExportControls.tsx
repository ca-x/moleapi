import {useLayoutEffect,useRef,useState} from "react";
import {Button,Callout,Checkbox,Flex,Text} from "@radix-ui/themes";
import {api,saveFile} from "../../shared/api";
import {errorCopy,type ErrorCopy} from "../../shared/i18n/errors";
import {t,translateCopy,useLanguage} from "../../shared/i18n";
import type {ExportResult} from "../../shared/types";
import {useWorkbench} from "../workbench/context";
import {hasChanges} from "../workspaces/draft";

export default function VariableFileExportControls(){
  useLanguage();const state=useWorkbench();
  const [includeSecrets,setIncludeSecrets]=useState(false),[pending,setPending]=useState(false),[problem,setProblem]=useState<ErrorCopy>("");
  const identity=JSON.stringify([state.accountId,state.draft?.id]);const latest=useRef(identity);latest.current=identity;
  const previous=useRef(identity);
  useLayoutEffect(()=>{if(previous.current!==identity){previous.current=identity;setIncludeSecrets(false);setProblem("");}},[identity]);
  async function download(scope:"environment"|"globals"){
    if(!state.draft||pending||state.saving)return;
    const origin=identity,current=()=>latest.current===origin;
    const workspace=state.draft.id,environmentId=scope==="environment"?state.environment?.id:null;
    if(scope==="environment"&&!environmentId)return;
    setPending(true);setProblem("");
    try{
      let revision=state.draft.revision;
      if(state.dirty){
        const saved=await state.save(true);
        if(!current())return;
        if(!saved||hasChanges(state.stateRef.current)){setProblem(t("工作区有未保存修改，请解决冲突后重试导出。"));return;}
        revision=saved.revision;
      }
      const file=await api<ExportResult>(`/api/workspaces/${workspace}/variables/export`,"POST",{
        scope,environment_id:environmentId,include_secrets:includeSecrets,expected_revision:revision
      });
      if(current())await saveFile(file);
    }catch(error){if(current())setProblem(errorCopy(error));}
    finally{setPending(false);}
  }
  return <Flex direction="column" gap="2">
    <Flex gap="2" align="center" wrap="wrap">
      <Button variant="soft" disabled={!state.environment||pending||state.saving} loading={pending} onClick={()=>void download("environment")}>{t("导出当前环境")}</Button>
      <Button variant="soft" disabled={pending||state.saving} onClick={()=>void download("globals")}>{t("导出项目全局变量")}</Button>
      <Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={includeSecrets} onCheckedChange={checked=>setIncludeSecrets(checked===true)}/>{t("包含私密变量值")}</Flex></Text>
    </Flex>
    {includeSecrets&&<Text size="1" color="amber">{t("完整导出包含敏感值，请妥善保管文件。")}</Text>}
    {problem&&<Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(problem)}</Callout.Text></Callout.Root>}
  </Flex>;
}
