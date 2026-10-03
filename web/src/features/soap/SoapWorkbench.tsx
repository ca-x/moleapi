import { useEffect, useRef, useState } from "react";
import { Badge, Button, Callout, Flex, Tabs, Text, TextField } from "@radix-ui/themes";
import { FileCode, RefreshCw, Server } from "lucide-react";
import { Choice, Editor, Field } from "../../shared/ui";
import type { RequestSpec } from "../../shared/types";
import { useWorkbench } from "../workbench/context";
import { ResponsePane } from "../requests/ResponsePane";
import { useSoapSchema } from "./useSoapSchema";
import type { SoapConfig } from "./types";
import SoapSourceDialog from "./SoapSourceDialog";
export default function SoapWorkbench() {
  const state = useWorkbench();
  const latest = useRef(state);
  latest.current = state;
  const source = useSoapSchema();
  const config =
    state.request?.protocol?.kind === "soap" ? state.request.protocol : null;
  const [sourceOpen, setSourceOpen] = useState(false);
  const [tab, setTab] = useState("xml");
  const identity = JSON.stringify([
    state.accountId,
    state.draft?.id,
    state.request?.id,
    state.draft?.data.active_environment_id,
  ]);
  useEffect(() => {
    setSourceOpen(false);
    setTab("xml");
  }, [identity]);
  function update(
    patch: Partial<SoapConfig>,
    request: Partial<RequestSpec> = {},
  ) {
    const current = latest.current.request?.protocol;
    if (current?.kind === "soap")
      latest.current.updateRequest({
        ...request,
        protocol: { ...current, ...patch },
      });
  }
  const service = source.schema?.services.find(
    (entry) => entry.name === config?.service,
  );
  const port = service?.ports.find((entry) => entry.name === config?.port);
  const operation = port?.operations.find(
    (entry) => entry.name === config?.operation,
  );
  function template() {
    if (!operation?.template || operation.error) return;
    const snapshot = {
      body: operation.template,
      body_kind: "text" as const,
      method: "POST",
    };
    const current = source.guard();
    const commit = () => {
      if (current()) latest.current.updateRequest(snapshot);
    };
    if (state.request?.body.trim() && state.request.body !== operation.template)
      state.setGuard({
        title: "替换 SOAP Envelope",
        description:
          "当前 XML 草稿会被所选操作的模板替换。保存的 WSDL/XSD 保持原样。",
        action: commit,
      });
    else commit();
  }
  if (!config || !state.request) return null;
  const response = state.response;
  return (
    <section className="soap-workbench" aria-label="SOAP 客户端">
      <div className="soap-toolbar">
        <Flex gap="3" align="center" wrap="wrap">
          <Choice
            value={state.request.specification_id || "none"}
            label="WSDL 服务定义"
            onChange={(value) =>
              latest.current.updateRequest({
                specification_id: value === "none" ? null : value,
                protocol: { ...config, service: "", port: "", operation: "" },
              })
            }
            options={[
              { value: "none", label: "手动 SOAP XML" },
              ...(state.draft?.data.specifications || [])
                .filter((entry) => entry.kind === "wsdl")
                .map((entry) => ({ value: entry.id, label: entry.name })),
            ]}
          />
          <Button
            size="1"
            color="gray"
            variant="soft"
            onClick={() => setSourceOpen(true)}
          >
            <FileCode size={14} />
            WSDL / XSD 来源
          </Button>
          <Choice
            value={config.version}
            label="SOAP 版本"
            onChange={(version) => update({ version })}
            options={[
              { value: "1.1", label: "SOAP 1.1" },
              { value: "1.2", label: "SOAP 1.2" },
            ]}
          />
          <Field label="SOAP Action">
            <TextField.Root
              value={config.action}
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
              onChange={(value) =>
                update({
                  service: value === "none" ? "" : value,
                  port: "",
                  operation: "",
                })
              }
              options={[
                { value: "none", label: "选择 Service" },
                ...source.schema.services.map((entry) => ({
                  value: entry.name,
                  label: entry.name,
                })),
              ]}
            />
            <Choice
              value={config.port || "none"}
              label="SOAP Port"
              disabled={!service}
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
                { value: "none", label: "选择 Port" },
                ...(service?.ports || []).map((entry) => ({
                  value: entry.name,
                  label: entry.name + " · " + entry.version,
                })),
              ]}
            />
            <Choice
              value={config.operation || "none"}
              label="SOAP Operation"
              disabled={!port}
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
                { value: "none", label: "选择 Operation" },
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
              disabled={!port?.address}
              onClick={() =>
                latest.current.updateRequest({ url: port!.address })
              }
            >
              <Server size={14} />
              使用 WSDL Endpoint
            </Button>
            <Button
              size="1"
              variant="soft"
              color="gray"
              disabled={!operation?.template || !!operation.error}
              onClick={template}
            >
              <RefreshCw size={14} />
              生成 XML 模板
            </Button>
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
        <Text size="1" color="gray" role="status">
          正在读取保存的 WSDL…
        </Text>
      )}
      <Tabs.Root value={tab} onValueChange={setTab} className="grpc-tabs">
        <Tabs.List>
          <Tabs.Trigger value="xml">SOAP Envelope</Tabs.Trigger>
          <Tabs.Trigger value="response">HTTP / XML 响应</Tabs.Trigger>
          <Tabs.Trigger value="operation">操作定义</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="xml">
          <Text size="1" color="gray">
            Action：{config.action || "未设置"} · 当前版本 SOAP {config.version}
          </Text>
          <Editor
            value={state.request.body}
            onChange={(body) => latest.current.updateRequest({ body })}
            dark={state.dark}
            language="xml"
            height="100%"
            label="SOAP XML 草稿"
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
            error={state.requestError}
            dark={state.dark}
            busy={state.sending}
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
              height="100%"
              label="SOAP 操作定义"
            />
          ) : (
            <Text color="gray">
              导入 WSDL 后选择 Service、Port 与 Operation。也可直接编写 XML
              调试已知 SOAP Endpoint。
            </Text>
          )}
        </Tabs.Content>
      </Tabs.Root>
      <SoapSourceDialog
        open={sourceOpen}
        onOpenChange={setSourceOpen}
        source={source}
      />
    </section>
  );
}
