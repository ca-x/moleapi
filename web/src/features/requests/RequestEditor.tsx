import ExamplesEditor from "./ExamplesEditor";
import AssertionsEditor from "../testing/AssertionsEditor";
import { ResponsePane } from "./ResponsePane";
import { useState } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import {
  Button,
  Checkbox,
  Flex,
  Tabs,
  Text,
  TextArea,
  TextField,
} from "@radix-ui/themes";
import { Braces, Copy, FileText, Save, Send } from "lucide-react";
import { toast } from "sonner";
import { Choice, Editor, Field, PairEditor, ToolButton } from "../../shared/ui";
import { curlTemplate, id, safeMessage } from "../../shared/model";
import type { ApiResponse, RequestSpec } from "../../shared/types";

const methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
const requestTabs = [
  { value: "query", label: "参数" },
  { value: "headers", label: "请求头" },
  { value: "body", label: "请求体" },
  { value: "auth", label: "鉴权" },
  { value: "assertions", label: "断言" },
  { value: "docs", label: "文档" },
  { value: "examples", label: "示例" },
  { value: "settings", label: "设置" },
];
export default function RequestEditor({
  request,
  update,
  send,
  save,
  dirty,
  saving,
  busy,
  sending,
  dark,
  response,
  error,
}: {
  request: RequestSpec;
  update: (patch: Partial<RequestSpec>) => void;
  send: () => void;
  save: () => void;
  dirty: boolean;
  saving: boolean;
  busy: boolean;
  sending: boolean;
  dark: boolean;
  response: ApiResponse | null;
  error: string;
}) {
  const [tab, setTab] = useState("query");
  const copyCurl = async () => {
    try {
      await navigator.clipboard.writeText(curlTemplate(request));
      toast.success("已复制 cURL 模板，鉴权值以占位符表示");
    } catch {
      toast.error("剪贴板不可用");
    }
  };
  const formatBody = () => {
    try {
      update({ body: JSON.stringify(JSON.parse(request.body), null, 2) });
    } catch (error) {
      toast.error(safeMessage(error));
    }
  };
  return (
    <div className="request-workspace">
      <header className="request-heading">
        <Flex direction="column" gap="1">
          <Text size="1" color="gray">
            HTTP 请求
          </Text>
          <TextField.Root
            className="request-title-input"
            variant="surface"
            aria-label="请求名称"
            value={request.name}
            onChange={(e) => update({ name: e.target.value })}
          />
        </Flex>
        <Flex gap="3" align="center">
          {dirty && (
            <Text size="1" color="gray">
              未保存
            </Text>
          )}
          <Button color="gray" variant="soft" loading={saving} onClick={save}>
            <Save size={15} />
            保存
          </Button>
          <ToolButton label="复制 cURL 模板" onClick={copyCurl}>
            <Copy size={17} />
          </ToolButton>
        </Flex>
      </header>
      <div className="url-toolbar">
        <Choice
          value={request.method}
          onChange={(method) => update({ method })}
          options={methods.map((value) => ({ value, label: value }))}
          label="HTTP 方法"
        />
        <TextField.Root
          className="url-input mono"
          size="3"
          aria-label="请求 URL"
          value={request.url}
          placeholder="https://api.example.com/v1/users"
          onChange={(e) => update({ url: e.target.value })}
        />
        <Button
          size="3"
          loading={busy}
          onClick={send}
          disabled={!request.url || sending}
        >
          <Send size={16} />
          发送
        </Button>
      </div>
      <Group
        id="request-response-split"
        orientation="vertical"
        className="request-response-split"
        resizeTargetMinimumSize={{ fine: 12, coarse: 44 }}
      >
        <Panel
          id="request-options-panel"
          defaultSize="35%"
          minSize="25%"
          className="request-options-panel"
        >
          <Tabs.Root
            value={tab}
            onValueChange={setTab}
            className="request-tabs"
          >
            <Tabs.List>
              {requestTabs.map((item) => (
                <Tabs.Trigger key={item.value} value={item.value}>
                  {item.label}
                  {(item.value === "query"
                    ? request.query.length
                    : item.value === "headers"
                      ? request.headers.length
                      : item.value === "assertions"
                        ? request.assertions.length
                        : 0) > 0 && (
                    <span className="count">
                      {item.value === "query"
                        ? request.query.length
                        : item.value === "headers"
                          ? request.headers.length
                          : request.assertions.length}
                    </span>
                  )}
                </Tabs.Trigger>
              ))}
            </Tabs.List>
            <Tabs.Content value="query">
              <PairEditor
                rows={request.query}
                onChange={(query) => update({ query })}
                keyLabel="参数名"
                valueLabel="参数值"
              />
            </Tabs.Content>
            <Tabs.Content value="headers">
              <PairEditor
                rows={request.headers}
                onChange={(headers) => update({ headers })}
                keyLabel="Header"
                valueLabel="值"
              />
            </Tabs.Content>
            <Tabs.Content value="body">
              <div className="body-options">
                <Choice
                  value={request.body_kind}
                  onChange={(body_kind) => update({ body_kind })}
                  options={[
                    { value: "none", label: "无请求体" },
                    { value: "json", label: "JSON" },
                    { value: "text", label: "Raw Text" },
                    { value: "form", label: "x-www-form-urlencoded" },
                  ]}
                  label="请求体格式"
                />
                {request.body_kind === "json" && (
                  <Button
                    size="1"
                    variant="ghost"
                    color="gray"
                    onClick={formatBody}
                  >
                    <Braces size={14} />
                    格式化
                  </Button>
                )}
              </div>
              {request.body_kind !== "none" ? (
                <Editor
                  value={request.body}
                  onChange={(body) => update({ body })}
                  dark={dark}
                  jsonMode={request.body_kind === "json"}
                  height="210px"
                />
              ) : (
                <Text size="2" color="gray">
                  此请求不发送 Body。
                </Text>
              )}
            </Tabs.Content>
            <Tabs.Content value="auth">
              <div className="form-panel">
                <Field label="鉴权类型">
                  <Choice
                    value={request.auth.kind}
                    onChange={(kind) =>
                      update({ auth: { ...request.auth, kind } })
                    }
                    options={[
                      { value: "none", label: "No Auth" },
                      { value: "bearer", label: "Bearer Token" },
                      { value: "basic", label: "Basic Auth" },
                    ]}
                    label="鉴权类型"
                  />
                </Field>
                {request.auth.kind === "bearer" && (
                  <Field
                    label="Token"
                    hint="可使用 {{api_token}} 引用环境变量。"
                  >
                    <TextField.Root
                      type="password"
                      autoComplete="off"
                      value={request.auth.token}
                      onChange={(e) =>
                        update({
                          auth: { ...request.auth, token: e.target.value },
                        })
                      }
                    />
                  </Field>
                )}
                {request.auth.kind === "basic" && (
                  <>
                    <Field label="用户名">
                      <TextField.Root
                        value={request.auth.username}
                        onChange={(e) =>
                          update({
                            auth: { ...request.auth, username: e.target.value },
                          })
                        }
                      />
                    </Field>
                    <Field label="密码">
                      <TextField.Root
                        type="password"
                        autoComplete="off"
                        value={request.auth.password}
                        onChange={(e) =>
                          update({
                            auth: { ...request.auth, password: e.target.value },
                          })
                        }
                      />
                    </Field>
                  </>
                )}
              </div>
            </Tabs.Content>
            <Tabs.Content value="assertions">
              <AssertionsEditor request={request} update={update} />
            </Tabs.Content>
            <Tabs.Content value="docs">
              <div className="documentation-editor">
                <Field label="接口说明">
                  <TextArea
                    rows={6}
                    value={request.description}
                    onChange={(e) => update({ description: e.target.value })}
                    placeholder="接口用途、参数约束与调用注意事项…"
                  />
                </Field>
                <Flex align="center" gap="2">
                  <FileText size={16} />
                  <Text size="2" color="gray">
                    说明随请求一起保存和导出。
                  </Text>
                </Flex>
              </div>
            </Tabs.Content>
            <Tabs.Content value="examples">
              <ExamplesEditor request={request} update={update} dark={dark} />
            </Tabs.Content>
            <Tabs.Content value="settings">
              <div className="form-panel">
                <Field label="请求超时 (ms)" hint="最大 120000ms。">
                  <TextField.Root
                    type="number"
                    min="100"
                    max="120000"
                    value={request.timeout_ms}
                    onChange={(e) =>
                      update({ timeout_ms: Number(e.target.value) })
                    }
                  />
                </Field>
                <label className="checkbox-label">
                  <Checkbox
                    checked={request.follow_redirects}
                    onCheckedChange={(v) =>
                      update({ follow_redirects: v === true })
                    }
                  />
                  自动跟随重定向
                </label>
                <label className="checkbox-label">
                  <Checkbox
                    checked={request.verify_tls}
                    onCheckedChange={(v) => update({ verify_tls: v === true })}
                  />
                  验证 TLS 证书
                </label>
              </div>
            </Tabs.Content>
          </Tabs.Root>
        </Panel>
        <Separator
          className="request-response-separator"
          aria-label="调整请求与响应面板高度"
        >
          <span aria-hidden="true" />
        </Separator>
        <Panel id="request-response-panel" defaultSize="65%" minSize="40%">
          <ResponsePane
            fill
            response={response}
            error={error}
            dark={dark}
            busy={busy}
            onExample={() => {
              if (response) {
                update({
                  examples: [
                    ...request.examples,
                    {
                      id: id(),
                      name: `${response.status} 示例`,
                      status: response.status,
                      headers: response.headers,
                      body: response.body,
                    },
                  ],
                });
                toast.success("示例已添加，保存后保留");
              }
            }}
          />
        </Panel>
      </Group>
    </div>
  );
}
