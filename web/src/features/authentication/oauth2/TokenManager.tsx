import {useEffect,useRef,useState} from "react";
import {Button,Callout,Dialog,Flex,Text,TextField,TextArea} from "@radix-ui/themes";
import {api,native} from "../../../shared/api";
import {t,useLanguage,translateCopy} from "../../../shared/i18n";
import {errorCopy,LocalizedError} from "../../../shared/i18n/errors";
import {Choice,Field} from "../../../shared/ui";
import {useWorkbench} from "../../workbench/context";
import type {OAuth2Auth,OAuth2Flow,OAuth2Token} from "./types";
export default function TokenManager({config,select,collectionId,redirect}:{config:OAuth2Auth;select:(id:string|null)=>void;collectionId?:string|null;redirect?:(url:string)=>void}) {
 useLanguage();const state=useWorkbench();const [open,setOpen]=useState(false),[cachedTokens,setTokens]=useState<OAuth2Token[]>([]),[cachedBusy,setBusy]=useState(false),[cachedFlow,assignFlow]=useState<OAuth2Flow|null>(null),[code,setCode]=useState(""),[label,setLabel]=useState(""),[cachedError,setError]=useState<ReturnType<typeof errorCopy>|null>(null);
 const [cachedSecret,setSecret]=useState<string|null>(null);
 const [importText,setImportText]=useState("");
 const [callbackMode,setCallbackMode]=useState("manual");
 const [inspection,setInspection]=useState<{active:boolean;expires_at:number|null;scopes:string[]}|null>(null);
 const identity=JSON.stringify([state.accountId,state.draft?.id,state.draft?.data.active_environment_id,state.request?.id,collectionId,config,open]);const current=useRef(identity);const generation=useRef(0);if(current.current!==identity){generation.current++;current.current=identity;}
 const [stateOwner,setStateOwner]=useState(identity);const owns=stateOwner===identity;
 const tokens=owns?cachedTokens:[],busy=owns&&cachedBusy,flow=owns?cachedFlow:null,error=owns?cachedError:null,secret=owns?cachedSecret:null;
 const activeFlow=useRef<{owner:string;id:string;stage:string}|null>(null);
 function setFlow(value:OAuth2Flow|null){activeFlow.current=value?{owner:identity,id:value.id,stage:value.stage}:null;assignFlow(value);}
 const running=useRef(false);
 const epoch=useRef(0);useEffect(()=>{epoch.current++;running.current=false;setStateOwner(identity);setTokens([]);setFlow(null);setSecret(null);setInspection(null);setCode("");setImportText("");setError(null);setBusy(false);},[identity]);
 useEffect(()=>()=>{const pending=activeFlow.current;if(pending?.owner===identity&&["pending","running"].includes(pending.stage)){void api(`/api/oauth2/flows/${encodeURIComponent(pending.id)}/cancel`,"POST").catch(()=>{});activeFlow.current=null;}},[identity]);
 const workspace=state.draft?.id;const base=workspace?`/api/workspaces/${encodeURIComponent(workspace)}/oauth2/tokens`:"";
 const collection = collectionId===undefined ? state.draft?.data.collections.find(c=>c.requests.some(r=>r.id===state.request?.id))?.id : collectionId;
 const contextReady = collection==null || !!state.draft?.data.collections.some(c=>c.id===collection);
 const input=()=>({workspace_id:workspace,config,label,environment_id:state.draft?.data.active_environment_id,collection_id:collection,locals:state.draft?state.localVariables.values(state.draft,collection??undefined,state.draft.data.active_environment_id):[],verify_tls:collectionId===undefined?(state.request?.verify_tls??true):true});
 const selected=tokens.find(token=>token.id===config.token_id);
 const needsMigration=selected?.profile_version!==undefined&&selected.profile_version!==2;
 const callbackUri=native?"http://127.0.0.1:49152/api/oauth2/callback":`${window.location.origin}/api/oauth2/callback`;

 async function load(owner=generation.current){if(generation.current!==owner)return;const list=await api<OAuth2Token[]>(base);if(generation.current===owner)setTokens(list);}
 useEffect(()=>{if(open&&workspace){const owner=generation.current;void load(owner).catch(e=>{if(generation.current===owner)setError(errorCopy(e));});}},[identity]);
 useEffect(()=>{if(!flow||!["pending","running"].includes(flow.stage))return;const owner=generation.current,id=flow.id;let stopped=false;let timer:ReturnType<typeof setTimeout>;
 const poll=async()=>{try{const next=await api<OAuth2Flow>(`/api/oauth2/flows/${encodeURIComponent(id)}`);if(stopped||generation.current!==owner)return;if(next.stage==="completed"){await load(owner);if(!stopped&&generation.current===owner){setFlow(next);select(next.token_id);}}else{setFlow(next);if(next.stage==="pending"||next.stage==="running")timer=setTimeout(poll,1500);}}catch(e){if(!stopped&&generation.current===owner)setError(errorCopy(e));}};
 timer=setTimeout(poll,1500);return()=>{stopped=true;clearTimeout(timer);};},[flow?.id,flow?.stage,identity]);
 async function action(operation:()=>Promise<void>){if(running.current||!workspace)return;running.current=true;const owner=generation.current,ticket=++epoch.current;setBusy(true);setError(null);try{await operation();}catch(e){if(generation.current===owner&&epoch.current===ticket)setError(errorCopy(e));}finally{if(generation.current===owner&&epoch.current===ticket){running.current=false;setBusy(false);}}}
 async function acquire(){const owner=generation.current;if(state.dirty&&!(await state.save(true)))return;if(generation.current!==owner)return;
 if(["client_credentials","password"].includes(config.grant)){const token=await api<OAuth2Token>("/api/oauth2/tokens/acquire","POST",input());if(generation.current!==owner)return;await load(owner);if(generation.current===owner)select(token.id);}
 else{const next=await api<OAuth2Flow>("/api/oauth2/flows","POST",{...input(),callback_mode:callbackMode});if(generation.current===owner){setCode("");setFlow(next);}else{void api(`/api/oauth2/flows/${encodeURIComponent(next.id)}/cancel`,"POST").catch(()=>{});}}
 }
 async function launch(url:string){const owner=generation.current;if(native){const {openUrl}=await import("@tauri-apps/plugin-opener");if(generation.current!==owner)return;await openUrl(url);}else window.open(url,"_blank","noopener,noreferrer");}
 async function complete(){if(!flow)return;const owner=generation.current;const redirect=new URL(code);const values=new URLSearchParams(config.grant==="implicit"?redirect.hash.slice(1):redirect.search);
 const stateValue=values.get("state");if(!stateValue)throw new LocalizedError("回调 URL 缺少 state");
 const implicit=config.grant==="implicit"?{access_token:values.get("access_token"),token_type:values.get("token_type"),expires_in:values.has("expires_in")?Number(values.get("expires_in")):undefined,scope:values.get("scope")??undefined}:undefined;
 const result=await api<OAuth2Flow>(`/api/oauth2/flows/${encodeURIComponent(flow.id)}/complete`,"POST",{state:stateValue,code:values.get("code"),implicit});if(generation.current!==owner)return;setFlow(result);setCode("");if(result.stage==="completed"){await load(owner);if(generation.current===owner)select(result.token_id);}
 }
 return <><Button variant="soft" color="gray" onClick={()=>setOpen(true)} disabled={!workspace||!contextReady}>{t("管理 OAuth2 Token")}</Button><Text size="1" color="gray">{config.token_id?t("已选择 Token：{{value0}}",{value0:config.token_id}):t("尚未选择 Token")}</Text>
 <Dialog.Root open={open} onOpenChange={setOpen}><Dialog.Content maxWidth="min(800px, calc(100vw - 24px))" onKeyDown={event=>{if(event.ctrlKey||event.metaKey)event.stopPropagation();}}>
 <Dialog.Title>{t("OAuth2 Token 管理")}</Dialog.Title><Dialog.Description>{t("凭据仅用于当前账户和工作区。授权后选择 Token，再发送请求。")}</Dialog.Description>
 <Flex direction="column" gap="3" mt="4"><Field label={t("Token 名称")}><TextField.Root value={label} onChange={e=>setLabel(e.target.value)} maxLength={128}/></Field>
 <Choice label={t("选择 Token")} value={config.token_id??"none"} options={[{value:"none",label:t("尚未选择 Token")},...tokens.map(token=>({value:token.id,label:`${token.label} · ${token.token_type}`}))]} onChange={id=>{setSecret(null);select(id==="none"?null:id);}}/>
 {needsMigration&&<Callout.Root color="orange" role="status"><Callout.Text>{t("令牌来自旧配置版本，请重新获取或导入；工作区和原始配置已保留。")}</Callout.Text></Callout.Root>}
 {selected&&<Text size="2" color={selected.revoked?"red":"gray"}>{selected.revoked?t("Token 已撤销"):selected.expires_at?t("Token 到期时间：{{value0}}",{value0:new Date(selected.expires_at*1000).toLocaleString()}):t("Token 未提供到期时间")}</Text>}
 {["authorization_code","implicit"].includes(config.grant)&&<>
 <Choice label={t("回调方式")} value={callbackMode} options={[{value:"manual",label:t("手动粘贴回调 URL")},{value:native?"loopback":"hosted",label:native?t("桌面本地自动回调"):t("服务端自动回调")}]} onChange={value=>{setCode("");setCallbackMode(value);}}/>
 {callbackMode!=="manual"&&<><Text size="2">{t("在授权服务中注册以下回调 URI，并设置到当前配置：")}</Text><Text className="mono" size="2">{callbackUri}</Text><Button variant="soft" color="gray" disabled={busy||!!flow&&["pending","running"].includes(flow.stage)} onClick={()=>redirect?.(callbackUri)}>{t("使用此回调 URI")}</Button></>}
 </>}
 <Flex gap="2" wrap="wrap"><Button disabled={busy||!!flow&&["pending","running"].includes(flow.stage)} onClick={()=>void action(acquire)}>{t("获取 Token")}</Button><Button variant="soft" disabled={busy||!config.token_id||selected?.revoked||needsMigration} onClick={()=>void action(async()=>{const owner=generation.current;await api(`${base}/${encodeURIComponent(config.token_id!)}/refresh`,"POST",input());await load(owner);})}>{t("刷新 Token")}</Button>
 <Button variant="soft" color="gray" disabled={busy||!config.token_id} onClick={()=>void action(async()=>{const owner=generation.current;const value=await api<{access_token:string}>(`${base}/${encodeURIComponent(config.token_id!)}/secret`);if(generation.current===owner)setSecret(value.access_token);})}>{t("显示 Token")}</Button>
 <Button variant="soft" color="red" disabled={busy||!config.token_id} onClick={()=>void action(async()=>{const owner=generation.current;await api(`${base}/${encodeURIComponent(config.token_id!)}`,"DELETE");if(generation.current===owner){select(null);setSecret(null);}await load(owner);})}>{t("删除 Token")}</Button></Flex>
 <Flex gap="2" wrap="wrap">
 <Button variant="soft" disabled={busy||!selected||!label.trim()} onClick={()=>void action(async()=>{const owner=generation.current;await api(`${base}/${encodeURIComponent(config.token_id!)}`,"PATCH",{label});await load(owner);})}>{t("重命名 Token")}</Button>
 <Button variant="soft" disabled={busy||!selected||selected.revoked||needsMigration||!config.introspection_url} onClick={()=>void action(async()=>{const owner=generation.current;const result=await api<{active:boolean;expires_at:number|null;scopes:string[]}>(`${base}/${encodeURIComponent(config.token_id!)}/introspect`,"POST",input());if(generation.current===owner)setInspection(result);})}>{t("检查 Token")}</Button>
 <Button variant="soft" color="red" disabled={busy||!selected||selected.revoked||needsMigration||!config.revocation_url} onClick={()=>void action(async()=>{const owner=generation.current;await api(`${base}/${encodeURIComponent(config.token_id!)}/revoke`,"POST",input());if(generation.current===owner)setSecret(null);await load(owner);})}>{t("撤销 Token")}</Button>
 </Flex>
 <details><summary>{t("导入已有 Token")}</summary><Field label={t("OAuth2 Token 响应 JSON")} hint={t("粘贴包含 access_token 和 token_type 的响应，可选 refresh_token、expires_in 和 scope。凭据仅存入当前账户的私有令牌库。") }><TextArea autoComplete="off" value={importText} maxLength={131072} onChange={e=>setImportText(e.target.value)}/></Field><Button variant="soft" disabled={busy||!importText} onClick={()=>void action(async()=>{const owner=generation.current;let token:unknown;try{token=JSON.parse(importText);}catch{throw new LocalizedError("Token 响应必须是有效 JSON");}if(state.dirty&&!(await state.save(true)))return;if(generation.current!==owner)return;const imported=await api<OAuth2Token>("/api/oauth2/tokens/import","POST",{...input(),token});if(generation.current===owner){setImportText("");await load(owner);if(generation.current===owner)select(imported.id);}})}>{t("导入 Token")}</Button></details>
 {inspection&&<Callout.Root color={inspection.active?"green":"orange"} role="status"><Callout.Text>{inspection.active?t("授权服务确认 Token 有效"):t("授权服务确认 Token 无效")}</Callout.Text></Callout.Root>}
 {secret&&<Field label="Access Token"><Flex gap="2"><TextField.Root readOnly value={secret} style={{flex:1}}/><Button variant="soft" color="gray" disabled={busy} onClick={()=>void action(()=>navigator.clipboard.writeText(secret))}>{t("复制 Token")}</Button></Flex></Field>}
 {flow&&<><Text size="2">{t("授权状态：{{value0}}",{value0:({pending:t("等待授权"),running:t("正在完成授权"),completed:t("授权完成"),cancelled:t("授权已取消"),failed:t("授权失败")} as Record<string,string>)[flow.stage]??flow.stage})}</Text>{flow.user_code&&<Text className="mono">{flow.user_code}</Text>}{(flow.authorization_url||flow.verification_uri)&&<Button variant="soft" disabled={busy} onClick={()=>void action(()=>launch((flow.authorization_url??flow.verification_uri)!))}>{t("在浏览器中授权")}</Button>}
 {config.grant!=="device_code"&&flow.stage==="pending"&&<><Field label={t("粘贴授权后的回调 URL")}><TextField.Root type="password" autoComplete="off" value={code} onChange={e=>setCode(e.target.value)}/></Field><Button disabled={busy||!code} onClick={()=>void action(complete)}>{t("完成授权")}</Button></>}
 {["pending","running"].includes(flow.stage)&&<Button variant="soft" color="gray" onClick={()=>{const owner=generation.current;void api<OAuth2Flow>(`/api/oauth2/flows/${encodeURIComponent(flow.id)}/cancel`,"POST").then(value=>{if(generation.current===owner)setFlow(value);}).catch(e=>{if(generation.current===owner)setError(errorCopy(e));});}}>{t("取消授权")}</Button>}{flow.error&&<Callout.Root color="red"><Callout.Text>{flow.error}</Callout.Text></Callout.Root>}</>}
 {error&&<Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
 <Flex justify="end"><Dialog.Close><Button variant="soft" color="gray">{t("完成")}</Button></Dialog.Close></Flex></Flex>
 </Dialog.Content></Dialog.Root></>;
}
