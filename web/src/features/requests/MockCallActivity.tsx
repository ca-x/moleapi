import {useEffect,useState} from "react";
import {Button,Flex,Text} from "@radix-ui/themes";
import {api,token} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";

interface MockCall {
  id:string;
  request_id:string|null;
  example_id:string|null;
  method:string;
  response_status:number;
  outcome:string;
  at:string;
  elapsed_ms:number;
}

export default function MockCallActivity({workspaceId,requestId}:{workspaceId:string;requestId:string}){
  useLanguage();
  const [loaded,setLoaded]=useState<{scope:string;identity:string|null;rows:MockCall[]}|null>(null);
  const [error,setError]=useState<{scope:string;identity:string|null}|null>(null);
  const [refresh,setRefresh]=useState(0);
  const [clearing,setClearing]=useState(false);
  const [confirmClear,setConfirmClear]=useState(false);
  const path=`/api/workspaces/${encodeURIComponent(workspaceId)}/mock-calls`;
  const scope=`${workspaceId}\0${requestId}`;
  const identity=token();
  const calls=loaded?.scope===scope&&loaded.identity===identity?loaded.rows:null;
  const currentError=error?.scope===scope&&error.identity===identity;
  useEffect(()=>{
    let active=true;
    setLoaded(null);setError(null);setConfirmClear(false);
    void api<MockCall[]>(path).then(rows=>{
      if(active&&token()===identity)setLoaded({scope,identity,rows:rows.filter(row=>row.request_id===requestId||row.request_id===null).slice(0,20)});
    }).catch(()=>{if(active&&token()===identity)setError({scope,identity});});
    return ()=>{active=false;};
  },[path,requestId,scope,identity,refresh]);
  const clear=async()=>{
    if(!confirmClear){setConfirmClear(true);return;}
    setClearing(true);setError(null);
    try{await api(path,"DELETE");setRefresh(value=>value+1);}
    catch{setError({scope,identity:token()});}
    finally{setClearing(false);setConfirmClear(false);}
  };
  return <section aria-label={t("Mock 调用日志")}>
    <Flex gap="2" align="center" wrap="wrap">
      <Text size="2" weight="medium">{t("Mock 调用日志")}</Text>
      <Button size="2" style={{minHeight:44}} variant="soft" color="gray" onClick={()=>setRefresh(value=>value+1)}>{t("刷新日志")}</Button>
      <Button size="2" style={{minHeight:44}} variant="ghost" color="red" disabled={clearing} onClick={()=>void clear()}>{confirmClear?t("确认清空工作区日志"):t("清空工作区日志")}</Button>
    </Flex>
    <Text as="p" size="1" color="gray">{t("仅保存匹配结果、状态和耗时；不保存请求内容。显示当前请求与未匹配调用。")}</Text>
    {currentError&&<Text role="alert" size="1" color="red">{t("无法读取或清空 Mock 日志。")}</Text>}
    {calls===null&&!currentError&&<Text size="1" color="gray">{t("正在读取日志…")}</Text>}
    {calls?.length===0&&<Text size="1" color="gray">{t("暂无 Mock 调用")}</Text>}
    {calls&&calls.length>0&&<ul className="mock-call-list">{calls.map(call=><li key={call.id}>
      <Text size="1">{call.method} · {call.response_status} · {call.outcome==="served"?t("已响应"):call.outcome==="unmatched"?t("未匹配"):call.outcome==="simulated_failure"?t("模拟故障"):call.outcome==="rate_limited"?t("已限流"):t("失败")} · {call.elapsed_ms} ms</Text>
      <Text size="1" color="gray">{call.example_id??"—"} · {new Date(call.at).toLocaleString()}</Text>
    </li>)}</ul>}
  </section>;
}
