import {Button, Flex, Text, TextField} from "@radix-ui/themes";
import {useState} from "react";
import {api} from "../../shared/api";
import {t, useLanguage} from "../../shared/i18n";
import type {DubboConfig, RequestSpec, Workspace, WorkspaceData} from "../../shared/types";
import {Choice, Field} from "../../shared/ui";

type Preview = {
  service:string; version:string; group:string; application:string;
  metadata_path:string; document_sha256:string; raw_document:string;
  methods:{name:string;parameter_types:string[];return_type:string}[];
};
const signature = (method:Preview["methods"][number]) => `${method.name}(${method.parameter_types.join(",")})`;

export default function MetadataPreviewPane({request, update, workspaceId, workspaceData, workspaceRevision, onImported, dirty, disabled}:{
  request:RequestSpec; update:(patch:Partial<RequestSpec>)=>void;
  workspaceId?:string; workspaceData?:WorkspaceData; workspaceRevision?:number;
  onImported?:(workspace:Workspace)=>void; dirty?:boolean; disabled:boolean;
}) {
  useLanguage();
  const [preview,setPreview] = useState<Preview|null>(null);
  const [error,setError] = useState("");
  const [pending,setPending] = useState(false);
  const [decisions,setDecisions] = useState<Record<string,string>>({});
  const [success,setSuccess] = useState("");
  if (request.protocol?.kind !== "dubbo") return null;
  const config = request.protocol;
  const change = (patch:Partial<DubboConfig>) => {
    setPreview(null); setError(""); setSuccess("");
    update({protocol:{...config,...patch}});
  };
  const changeAuth = (username:string,password:string) =>
    change({metadata_auth:username || password ? {username,password} : undefined});
  const open = () => {
    if (!workspaceId || pending) return;
    setPending(true); setError(""); setPreview(null); setDecisions({}); setSuccess("");
    void api<Preview>(`/api/workspaces/${encodeURIComponent(workspaceId)}/dubbo/metadata/preview`,"POST",{request})
      .then(setPreview).catch(reason=>setError(reason instanceof Error ? reason.message : String(reason)))
      .finally(()=>setPending(false));
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
  const hasSelected = preview?.methods.some(method=>selected(method)!=="skip") ?? false;
  const apply = () => {
    if (!preview || !workspaceId || !collection || workspaceRevision===undefined || !onImported || pending || dirty) return;
    const choices = preview.methods.map(method=>{
      const value = selected(method);
      return {signature:signature(method),action:value.startsWith("replace:")?"replace":value,
        replace_request_id:value.startsWith("replace:")?value.slice(8):undefined};
    });
    setPending(true); setError(""); setSuccess("");
    void api<Workspace>(`/api/workspaces/${encodeURIComponent(workspaceId)}/dubbo/metadata/import`,"POST",{
      request,collection_id:collection.id,expected_revision:workspaceRevision,
      document_sha256:preview.document_sha256,decisions:choices,
    }).then(workspace=>{onImported(workspace);setPreview(null);setSuccess(t("已导入所选方法"));})
      .catch(reason=>setError(reason instanceof Error ? reason.message : String(reason)))
      .finally(()=>setPending(false));
  };
  return <div className="form-panel">
    <Text weight="medium">{t("Dubbo 元数据中心")}</Text>
    <Text size="2" color="gray">{t("填写提供方使用的 ZooKeeper 元数据中心及 application 名称。预览不会修改工作区。")}</Text>
    <Field label={t("元数据中心地址")}><TextField.Root className="mono" style={{width:"100%"}} aria-label={t("元数据中心地址")} value={config.metadata_center_url ?? ""} maxLength={2048} disabled={disabled} placeholder="zookeeper://host:2181" onChange={event=>change({metadata_center_url:event.target.value})}/></Field>
    <Field label={t("提供方 application")}><TextField.Root style={{width:"100%"}} aria-label={t("提供方 application")} value={config.metadata_application ?? ""} maxLength={128} disabled={disabled} placeholder="provider-app" onChange={event=>change({metadata_application:event.target.value})}/></Field>
    <Flex gap="3" wrap="wrap">
      <Field label={t("元数据用户名")}><TextField.Root aria-label={t("元数据用户名")} value={config.metadata_auth?.username ?? ""} maxLength={128} disabled={disabled} onChange={event=>changeAuth(event.target.value,config.metadata_auth?.password ?? "")}/></Field>
      <Field label={t("元数据密码")}><TextField.Root type="password" aria-label={t("元数据密码")} value={config.metadata_auth?.password ?? ""} maxLength={4096} disabled={disabled} onChange={event=>changeAuth(config.metadata_auth?.username ?? "",event.target.value)}/></Field>
    </Flex>
    <Button size="2" variant="soft" loading={pending} disabled={disabled||!workspaceId||!config.metadata_center_url||!config.metadata_application} onClick={open}>{t("预览元数据")}</Button>
    {error && <Text size="2" color="red" role="alert">{error}</Text>}
    {success && <Text size="2" color="green" role="status">{success}</Text>}
    {preview && <div>
      <Text size="2">{preview.service} · {preview.version} · {preview.group} · {preview.application}</Text>
      {preview.methods.map((method,index)=><div key={`${method.name}-${index}`} style={{marginTop:12}}>
        <Text as="p" size="2">{method.name}({method.parameter_types.join(", ")}) → {method.return_type}</Text>
        <Choice value={selected(method)} disabled={disabled||pending} label={`${t("导入决策")}: ${method.name}`}
          options={[
            {value:"skip",label:t("跳过")},
            {value:"add",label:conflicts(method).length?t("另建接口"):t("添加接口")},
            ...conflicts(method).map(row=>({value:`replace:${row.id}`,label:`${t("替换")} ${row.name}`})),
          ]} onChange={value=>setDecisions(current=>({...current,[signature(method)]:value}))}/>
      </div>)}
      {dirty && <Text as="p" size="2" color="gray">{t("先保存当前请求，再导入元数据。")}</Text>}
      <Button size="2" style={{marginTop:12}} loading={pending} disabled={disabled||dirty||!hasSelected||!workspaceId||!collection||workspaceRevision===undefined||!onImported||pending} onClick={apply}>{t("导入所选方法")}</Button>
      <details><summary>{t("原始元数据")}</summary><pre className="mono" style={{maxHeight:240,overflow:"auto",whiteSpace:"pre-wrap",overflowWrap:"anywhere"}}>{preview.raw_document}</pre></details>
    </div>}
  </div>;
}
