import { t, useLanguage } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { AlertDialog, Badge, Button, Callout, Flex, Tabs, Text, TextField } from "@radix-ui/themes";
import { FileCode, RefreshCw, Server } from "lucide-react";
import { Choice, Editor, Field } from "../../shared/ui";
import type { ApiResponse, Specification, RequestSpec } from "../../shared/types";
import { ResponsePane } from "../requests/ResponsePane";
import type { SoapConfig, SoapSchema } from "./types";
export default function SoapClient(state: {
  scope:string; request:RequestSpec; specifications:Specification[]; schema:SoapSchema|null; dark:boolean;
  onChange:(patch:Partial<RequestSpec>)=>void; isCurrent:()=>boolean; disabled?:boolean;
  sourceError?:string; sourceBusy?:boolean; sourceActionsDisabled?:boolean; onImport:()=>void;
  response?:ApiResponse|null; requestError?:string; sending?:boolean; onSend?:()=>void; sendDisabled?:boolean; sendLabel?:string;
}) {
  useLanguage();
  const latest=useRef(state); latest.current=state;
  const identity=JSON.stringify([state.scope,state.request]); const scope=useRef(identity);scope.current=identity;
  const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const [pendingTemplate,setPendingTemplate]=useState<{identity:string;template:string}|null>(null);
  const [tab,setTab]=useState("xml");
  useEffect(()=>{setPendingTemplate(null);},[identity]);
  function current(origin=identity) { return mounted.current && scope.current===origin && !latest.current.disabled && latest.current.isCurrent(); }
  function change(patch:Partial<RequestSpec>) { if(current()) latest.current.onChange(patch); }
  function update(patch:Partial<SoapConfig>, request:Partial<RequestSpec>={}) { const config=latest.current.request.protocol; if(config?.kind==="soap") change({...request,protocol:{...config,...patch}}); }
  const config=state.request.protocol?.kind==="soap"?state.request.protocol:null;
  const source={schema:state.schema,error:state.sourceError,busy:state.sourceBusy};
  const service = source.schema?.services.find(
    (entry) => entry.name === config?.service,
  );
  const port = service?.ports.find((entry) => entry.name === config?.port);
  const operation = port?.operations.find(
    (entry) => entry.name === config?.operation,
  );
  function applyTemplate(template:string,origin=identity) { if(current(origin)) latest.current.onChange({body:template,body_kind:"text",method:"POST"}); }
  function template() {
    if(!current() || !operation?.template || operation.error) return;
    if(state.request.body.trim() && state.request.body!==operation.template) setPendingTemplate({identity,template:operation.template});
    else applyTemplate(operation.template);
  }
  function send() { if(current() && !latest.current.sendDisabled && !latest.current.sending) latest.current.onSend?.(); }
  if (!config || !state.request) return null;
  const response = state.response ?? null;
  return (
    <section className="soap-workbench" aria-label={t("SOAP 客户端")} onKeyDown={(event)=>{if(state.onSend && (event.ctrlKey||event.metaKey) && event.key==="Enter"){event.preventDefault();send();}}}>
      <div className="soap-toolbar">
        <Flex gap="3" align="center" wrap="wrap">
          <Choice
            value={state.request.specification_id || "none"}
            label={t("WSDL 服务定义")}
            disabled={state.disabled}
            onChange={(value)=>update({service:"",port:"",operation:"",action:""},{specification_id:value==="none"?null:value})}
            options={[
              { value: "none", label: t("手动 SOAP XML") },
              ...state.specifications
                .filter((entry) => entry.kind === "wsdl")
                .map((entry) => ({ value: entry.id, label: entry.name })),
            ]}
          />
          <Button
            size="1"
            color="gray"
            variant="soft"
            disabled={state.disabled || state.sourceActionsDisabled}
            onClick={() => { if(current()) state.onImport(); }}
          >
            <FileCode size={14} /> {t("WSDL / XSD 来源")} </Button>
          <Choice
            value={config.version}
            label={t("SOAP 版本")}
            disabled={state.disabled}
            onChange={(version) => update({ version })}
            options={[
              { value: "1.1", label: "SOAP 1.1" },
              { value: "1.2", label: "SOAP 1.2" },
            ]}
          />
          <Field label="SOAP Action">
            <TextField.Root
              value={config.action}
              disabled={state.disabled}
              readOnly={!!operation}
              onChange={(event) => update({ action: event.target.value })}
              placeholder="urn:operation"
            />
          </Field>
          {response && (
            <Badge color={response.status < 400 ? "green" : "red"}>
              HTTP {response.status}
            </Badge>
          )}
          {response?.soap_fault && <Badge color="red">SOAP Fault</Badge>}
        </Flex>
        {source.schema && (
          <Flex gap="3" align="center" wrap="wrap" mt="3">
            <Choice
              value={config.service || "none"}
              label="SOAP Service"
              disabled={state.disabled}
              onChange={(value) =>
                update({
                  service: value === "none" ? "" : value,
                  port: "",
                  operation: "",
                })
              }
              options={[
                { value: "none", label: t("选择 Service") },
                ...source.schema.services.map((entry) => ({
                  value: entry.name,
                  label: entry.name,
                })),
              ]}
            />
            <Choice
              value={config.port || "none"}
              label="SOAP Port"
              disabled={state.disabled || !service}
              onChange={(value) => {
                const selected = service?.ports.find(
                  (entry) => entry.name === value,
                );
                update({
                  port: value === "none" ? "" : value,
                  operation: "",
                  ...(selected ? { version: selected.version } : {}),
                });
              }}
              options={[
                { value: "none", label: t("选择 Port") },
                ...(service?.ports || []).map((entry) => ({
                  value: entry.name,
                  label: entry.name + " · " + entry.version,
                })),
              ]}
            />
            <Choice
              value={config.operation || "none"}
              label="SOAP Operation"
              disabled={state.disabled || !port}
              onChange={(value) => {
                const selected = port?.operations.find(
                  (entry) => entry.name === value,
                );
                update({
                  operation: value === "none" ? "" : value,
                  action: selected?.action || "",
                });
              }}
              options={[
                { value: "none", label: t("选择 Operation") },
                ...(port?.operations || []).map((entry) => ({
                  value: entry.name,
                  label: entry.name,
                })),
              ]}
            />
            <Button
              size="1"
              variant="soft"
              color="gray"
              disabled={state.disabled || !port?.address}
              onClick={() =>
                change({ url: port!.address })
              }
            >
              <Server size={14} /> {t("使用 WSDL Endpoint")} </Button>
            <Button
              size="1"
              variant="soft"
              color="gray"
              disabled={state.disabled || !operation?.template || !!operation.error}
              onClick={template}
            >
              <RefreshCw size={14} /> {t("生成 XML 模板")} </Button>
          </Flex>
        )}
      </div>
      {state.requestError && (
        <Callout.Root color="red" role="alert">
          <Callout.Text>{state.requestError}</Callout.Text>
        </Callout.Root>
      )}
      {(source.error || operation?.error) && (
        <Callout.Root color="red" role="alert">
          <Callout.Text>{source.error || operation?.error}</Callout.Text>
        </Callout.Root>
      )}
      {source.busy && (
        <Text size="1" color="gray" role="status"> {t("正在读取保存的 WSDL…")} </Text>
      )}
      <Tabs.Root value={tab} onValueChange={setTab} className="grpc-tabs">
        <Tabs.List>
          <Tabs.Trigger value="xml">SOAP Envelope</Tabs.Trigger>
          <Tabs.Trigger value="response">{t("HTTP / XML 响应")}</Tabs.Trigger>
          <Tabs.Trigger value="operation">{t("操作定义")}</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="xml">
          <Text size="1" color="gray">
            Action：{config.action || t("未设置")} {t("· 当前版本 SOAP")} {config.version}
          </Text>
          <Editor
            value={state.request.body}
            onChange={(body) => change({ body })}
            readOnly={state.disabled}
            dark={state.dark}
            language="xml"
            height="240px"
            label={t("SOAP XML 草稿")}
          />
        </Tabs.Content>
        <Tabs.Content value="response">
          {response?.soap_fault && (
            <Callout.Root color="red" role="alert">
              <Callout.Text>
                {response.soap_fault.code + ": " + response.soap_fault.reason}
              </Callout.Text>
              {response.soap_fault.detail && (
                <Editor
                  value={response.soap_fault.detail}
                  dark={state.dark}
                  language="xml"
                  readOnly
                  height="160px"
                  label="SOAP Fault Detail"
                />
              )}
            </Callout.Root>
          )}
          <ResponsePane
            fill
            response={response}
            error={state.requestError ?? ""}
            dark={state.dark}
            busy={!!state.sending}
          />
        </Tabs.Content>
        <Tabs.Content value="operation">
          {operation ? (
            <Editor
              value={JSON.stringify(
                {
                  service: service?.name,
                  port: port?.name,
                  binding: port?.binding,
                  version: port?.version,
                  address: port?.address,
                  operation,
                },
                null,
                2,
              )}
              dark={state.dark}
              jsonMode
              readOnly
              height="240px"
              label={t("SOAP 操作定义")}
            />
          ) : (
            <Text color="gray"> {t("导入 WSDL 后选择 Service、Port 与 Operation。也可直接编写 XML 调试已知 SOAP Endpoint。")} </Text>
          )}
        </Tabs.Content>
      </Tabs.Root>
      {state.onSend && <Button loading={state.sending} disabled={state.disabled || state.sendDisabled || state.sending} onClick={send}>{state.sendLabel}</Button>}
      <AlertDialog.Root open={pendingTemplate?.identity===identity} onOpenChange={open=>{if(!open)setPendingTemplate(null);}}><AlertDialog.Content><AlertDialog.Title>{t("替换 SOAP Envelope")}</AlertDialog.Title><AlertDialog.Description>{t("当前 XML 草稿会被所选操作的模板替换。保存的 WSDL/XSD 保持原样。")}</AlertDialog.Description><Flex gap="3" mt="4" justify="end"><AlertDialog.Cancel><Button variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel><Button onClick={()=>{const pending=pendingTemplate;setPendingTemplate(null);if(pending)applyTemplate(pending.template,pending.identity);}}>{t("替换")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
    </section>
  );
}
