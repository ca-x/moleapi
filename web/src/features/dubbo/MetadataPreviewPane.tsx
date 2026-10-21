import {Button, Flex, Text, TextField} from "@radix-ui/themes";
import {useEffect,useRef,useState} from "react";
import {errorCopy,type ErrorCopy} from "../../shared/i18n/errors";
import {api} from "../../shared/api";
import {t, useLanguage, translateCopy} from "../../shared/i18n";
import type {DubboConfig, RequestSpec, Workspace, WorkspaceData} from "../../shared/types";
import {Choice, Field} from "../../shared/ui";

export type Preview = {
  service:string; version:string; group:string; application:string;
  metadata_path:string; document_sha256:string; raw_document:string;
  methods:{name:string;parameter_types:string[];return_type:string}[];
  types:{type_name:string;properties:{name:string;type_name:string}[];items:string[];enums:string[]}[];
};
const signature = (method:Preview["methods"][number]) => `${method.name}(${method.parameter_types.join(",")})`;

export type MetadataDecision={signature:string;action:"skip"|"add"|"replace";replace_request_id?:string};
export interface MetadataOperations {ready:boolean;isCurrent:()=>boolean;preview:()=>Promise<Preview>;importMetadata:(sha:string,decisions:MetadataDecision[])=>Promise<Workspace>}
export default function MetadataPreviewPane(props:{request:RequestSpec;update:(patch:Partial<RequestSpec>)=>void;workspaceId?:string;workspaceData?:WorkspaceData;workspaceRevision?:number;onImported?:(workspace:Workspace)=>void;dirty?:boolean;disabled:boolean;scope?:string;isCurrent?:()=>boolean;operations?:MetadataOperations}) {
  useLanguage();const {request,update,workspaceId,workspaceData,workspaceRevision,onImported,dirty,disabled,operations}=props;
  const identity=JSON.stringify([props.scope,workspaceId,workspaceRevision,request,workspaceData,dirty,disabled,operations?.ready]);const scope=useRef(identity),epoch=useRef(0);if(scope.current!==identity){scope.current=identity;epoch.current++;}const generation=epoch.current;
  const latest=useRef(props);latest.current=props;const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const [loaded,setLoaded]=useState<{identity:string;generation:number;value:Preview}|null>(null),[failure,setFailure]=useState<{identity:string;generation:number;value:ErrorCopy}|null>(null),[active,setActive]=useState<{identity:string;generation:number}|null>(null),[decisions,setDecisions]=useState<Record<string,string>>({}),[success,setSuccess]=useState<string|null>(null);
  const preview=loaded?.identity===identity&&loaded.generation===generation?loaded.value:null,error=failure?.identity===identity&&failure.generation===generation?translateCopy(failure.value):"",pending=active?.identity===identity&&active.generation===generation;
  function admitted(){return mounted.current&&scope.current===identity&&epoch.current===generation&&!latest.current.disabled&&(latest.current.isCurrent?.()??true)&&(latest.current.operations?.isCurrent()??true);}
  const ready=!operations||operations.ready,successScope=JSON.stringify([props.scope,workspaceId,request.id]);
  function fail(reason:unknown){if(admitted())setFailure({identity,generation,value:errorCopy(reason)});}
  function finish(){if(mounted.current&&scope.current===identity&&epoch.current===generation)setActive(null);}
  if (request.protocol?.kind !== "dubbo") return null;
  const config = request.protocol;
  const nacosMetadata = config.metadata_center_url?.toLowerCase().startsWith("nacos://") ?? false;
  const change = (patch:Partial<DubboConfig>) => {
    if(!admitted())return;setLoaded(null); setFailure(null); setSuccess(null);
    update({protocol:{...config,...patch}});
  };
  const changeAuth = (username:string,password:string) =>
    change({metadata_auth:username || password ? {username,password} : undefined});
  const changeEndpoint = (value:string) => {
    const patch:Partial<DubboConfig> = {metadata_center_url:value};
    try {
      const next = new URL(value);
      if (next.protocol === "zookeeper:" && next.host) {
        patch.metadata_namespace = "";
        patch.metadata_group = "";
      }
      try {
        const current = new URL(config.metadata_center_url ?? "");
        if (next.protocol !== current.protocol || next.host !== current.host) patch.metadata_auth = undefined;
      } catch { /* The previous endpoint is incomplete. */ }
    } catch { /* Keep the current source while the endpoint is being edited. */ }
    change(patch);
  };
  const open=()=>{
    if(!admitted()||!ready||(!workspaceId&&!operations)||pending)return;
    setActive({identity,generation});setFailure(null);setLoaded(null);setDecisions({});setSuccess(null);
    const operation=operations?operations.preview():api<Preview>(`/api/workspaces/${encodeURIComponent(workspaceId!)}/dubbo/metadata/preview`,"POST",{request});
    void operation.then(value=>{if(admitted())setLoaded({identity,generation,value});}).catch(fail).finally(finish);
  };
  const collection = workspaceData?.collections.find(item=>item.requests.some(row=>row.id===request.id));
  const conflicts = (method:Preview["methods"][number]) => collection?.requests.filter(row=>{
    if(row.protocol?.kind !== "dubbo") return false;
    return row.protocol.service===config.service && row.protocol.version===config.version
      && row.protocol.group===config.group && row.protocol.method===method.name
      && row.protocol.arguments.map(arg=>arg.type_name).join(",")===method.parameter_types.join(",");
  }) ?? [];
  const selected = (method:Preview["methods"][number]) =>
    decisions[signature(method)] ?? (conflicts(method).length ? "skip" : "add");
  const relatedTypes = (method:Preview["methods"][number]) => {
    const definitions = new Map(preview?.types.map(type=>[type.type_name,type]));
    const queue = [...method.parameter_types,method.return_type];
    const seen = new Set<string>();
    const linked:Preview["types"] = [];
    while(queue.length && seen.size<64) {
      const name = queue.shift()!;
      if(seen.has(name)) continue;
      seen.add(name);
      const definition = definitions.get(name);
      if(!definition) continue;
      if(definition.properties.length || definition.items.length || definition.enums.length) linked.push(definition);
      queue.push(...definition.properties.map(property=>property.type_name),...definition.items);
    }
    return {linked,truncated:queue.length>0};
  };
  const hasSelected = preview?.methods.some(method=>selected(method)!=="skip") ?? false;
  const apply=()=>{
    if(!admitted()||!ready||!preview||(!workspaceId&&!operations)||!collection||workspaceRevision===undefined||!onImported||pending||dirty)return;
    const choices:MetadataDecision[]=preview.methods.map(method=>{const value=selected(method);return {signature:signature(method),action:value.startsWith("replace:")?"replace":value==="add"?"add":"skip",replace_request_id:value.startsWith("replace:")?value.slice(8):undefined};});
    setActive({identity,generation});setFailure(null);setSuccess(null);
    const operation=operations?operations.importMetadata(preview.document_sha256,choices):api<Workspace>(`/api/workspaces/${encodeURIComponent(workspaceId!)}/dubbo/metadata/import`,"POST",{request,collection_id:collection.id,expected_revision:workspaceRevision,document_sha256:preview.document_sha256,decisions:choices});
    void operation.then(workspace=>{if(admitted()){onImported(workspace);setLoaded(null);setSuccess(successScope);}}).catch(fail).finally(finish);
  };
  return <div className="form-panel">
    <Text weight="medium">{t("Dubbo 元数据中心")}</Text>
    <Text size="2" color="gray">{t("填写提供方使用的元数据中心及 application。Nacos 命名空间留空表示 public，配置分组留空表示 dubbo。")}</Text>
    <Field label={t("元数据中心地址")}><TextField.Root className="mono" style={{width:"100%"}} aria-label={t("元数据中心地址")} value={config.metadata_center_url ?? ""} maxLength={2048} disabled={disabled} placeholder="zookeeper://host:2181 or nacos://host:8848" onChange={event=>changeEndpoint(event.target.value)}/></Field>
    <Field label={t("提供方 application")}><TextField.Root style={{width:"100%"}} aria-label={t("提供方 application")} value={config.metadata_application ?? ""} maxLength={128} disabled={disabled} placeholder="provider-app" onChange={event=>change({metadata_application:event.target.value})}/></Field>
    {nacosMetadata && <Flex gap="3" wrap="wrap">
      <Field label={t("元数据命名空间 ID")}><TextField.Root aria-label={t("元数据命名空间 ID")} value={config.metadata_namespace ?? ""} maxLength={128} disabled={disabled} placeholder="public" onChange={event=>change({metadata_namespace:event.target.value})}/></Field>
      <Field label={t("元数据配置分组")}><TextField.Root aria-label={t("元数据配置分组")} value={config.metadata_group ?? ""} maxLength={128} disabled={disabled} placeholder="dubbo" onChange={event=>change({metadata_group:event.target.value})}/></Field>
    </Flex>}
    <Flex gap="3" wrap="wrap">
      <Field label={t("元数据用户名")}><TextField.Root aria-label={t("元数据用户名")} value={config.metadata_auth?.username ?? ""} maxLength={128} disabled={disabled} onChange={event=>changeAuth(event.target.value,config.metadata_auth?.password ?? "")}/></Field>
      <Field label={t("元数据密码")}><TextField.Root type="password" aria-label={t("元数据密码")} value={config.metadata_auth?.password ?? ""} maxLength={4096} disabled={disabled} onChange={event=>changeAuth(config.metadata_auth?.username ?? "",event.target.value)}/></Field>
    </Flex>
    <Button size="2" variant="soft" loading={pending} disabled={disabled||!ready||(!workspaceId&&!operations)||!config.metadata_center_url||!config.metadata_application} onClick={open}>{t("预览元数据")}</Button>
    {error && <Text size="2" color="red" role="alert">{error}</Text>}
    {success===successScope && <Text size="2" color="green" role="status">{t("已导入所选方法")}</Text>}
    {preview && <div>
      <Text size="2">{preview.service} · {preview.version} · {preview.group} · {preview.application}</Text>
      {preview.methods.map((method,index)=>{
        const {linked,truncated} = relatedTypes(method);
        return <div key={`${method.name}-${index}`} style={{marginTop:12}}>
        <Text as="p" size="2">{method.name}({method.parameter_types.join(", ")}) → {method.return_type}</Text>
        {!!linked.length && <details>
          <summary>{t("类型映射")} ({linked.length})</summary>
          {linked.map(type=><Text key={type.type_name} as="p" size="2" color="gray" style={{overflowWrap:"anywhere"}}>
            {type.type_name}: {[
              ...type.properties.map(property=>`${property.name}: ${property.type_name}`),
              ...(type.items.length?[`${t("元素类型")}: ${type.items.join(", ")}`]:[]),
              ...(type.enums.length?[`${t("枚举值")}: ${type.enums.join(", ")}`]:[]),
            ].join("; ")}
          </Text>)}
          {truncated && <Text as="p" size="2" color="gray">{t("更多类型见原始元数据")}</Text>}
        </details>}
        <Choice value={selected(method)} disabled={disabled||pending} label={`${t("导入决策")}: ${method.name}`}
          options={[
            {value:"skip",label:t("跳过")},
            {value:"add",label:conflicts(method).length?t("另建接口"):t("添加接口")},
            ...conflicts(method).map(row=>({value:`replace:${row.id}`,label:`${t("替换")} ${row.name}`})),
          ]} onChange={value=>{if(admitted()&&!pending)setDecisions(current=>({...current,[signature(method)]:value}));}}/>
      </div>})}
      {dirty && <Text as="p" size="2" color="gray">{t("先保存当前请求，再导入元数据。")}</Text>}
      <Button size="2" style={{marginTop:12}} loading={pending} disabled={disabled||!ready||dirty||!hasSelected||(!workspaceId&&!operations)||!collection||workspaceRevision===undefined||!onImported||pending} onClick={apply}>{t("导入所选方法")}</Button>
      <details><summary>{t("原始元数据")}</summary><pre className="mono" style={{maxHeight:240,overflow:"auto",whiteSpace:"pre-wrap",overflowWrap:"anywhere"}}>{preview.raw_document}</pre></details>
    </div>}
  </div>;
}
