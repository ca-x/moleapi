import ScriptConsole from "../scripts/ScriptConsole";
import { useState } from "react";
import {
  Badge,
  Button,
  Callout,
  Flex,
  Heading,
  Table,
  Tabs,
  Text,
} from "@radix-ui/themes";
import { Check, Copy, Save, Send, X } from "lucide-react";
import { toast } from "sonner";
import { Editor, ToolButton } from "../../shared/ui";
import { bytes } from "../../shared/model";
import type { ApiResponse } from "../../shared/types";
export function ResponsePane({
  response,
  error,
  dark,
  busy,
  onExample,
  fill = false,
}: {
  response: ApiResponse | null;
  error: string;
  dark: boolean;
  busy: boolean;
  onExample?: () => void;
  fill?: boolean;
}) {
  const [pretty, setPretty] = useState(true);
  let content = response?.body || "";
  let isJson = false;
  try {
    const parsed = JSON.parse(content);
    isJson = true;
    if (pretty) content = JSON.stringify(parsed, null, 2);
  } catch {
    /* Non-JSON remains readable text. */
  }
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(response?.body || "");
      toast.success("已复制响应");
    } catch {
      toast.error("无法访问剪贴板，请在编辑器中选择文本复制。");
    }
  };
  return (
    <section
      className={`response-pane ${fill ? "response-pane-fill" : ""}`}
      aria-label="响应面板"
    >
      <div className="response-toolbar">
        <Flex gap="3" align="center">
          <Text weight="medium" size="2">
            响应
          </Text>
          {response && (
            <>
              <Badge color={response.status < 400 ? "green" : "red"}>
                {response.status} {response.status_text}
              </Badge>
              <Text size="1" color="gray" className="mono">
                {response.elapsed_ms} ms
              </Text>
              <Text size="1" color="gray" className="mono">
                {bytes(response.size_bytes)}
              </Text>
            </>
          )}
        </Flex>
        {response && (
          <Flex gap="3">
            <ToolButton label="复制响应" onClick={copy}>
              <Copy size={15} />
            </ToolButton>
            {onExample && (
              <Button variant="ghost" size="1" color="gray" onClick={onExample}>
                <Save size={14} />
                保存示例
              </Button>
            )}
          </Flex>
        )}
      </div>
      {error && (
        <Callout.Root color="red" role="alert" className="response-error">
          <Callout.Text>{error}</Callout.Text>
        </Callout.Root>
      )}
      {busy && (
        <div className="response-loading" role="status">
          正在等待响应…
        </div>
      )}
      {!response && !error && !busy && (
        <div className="response-empty">
          <Send size={30} strokeWidth={1.3} />
          <Heading size="3">发送请求，查看响应</Heading>
          <Text size="2" color="gray">
            状态、耗时、响应内容和测试结果会显示在这里。
          </Text>
          <Text size="1" color="gray">
            Ctrl / ⌘ + Enter
          </Text>
        </div>
      )}
      {response && (
        <Tabs.Root defaultValue="body">
          <Tabs.List>
            <Tabs.Trigger value="body">响应体</Tabs.Trigger>
            <Tabs.Trigger value="headers">
              响应头 <span className="count">{response.headers.length}</span>
            </Tabs.Trigger>
            <Tabs.Trigger value="tests">
              测试 <span className="count">{response.tests.length}</span>
            </Tabs.Trigger>
            <Tabs.Trigger value="logs">
              控制台 <span className="count">{response.logs?.length || 0}</span>
            </Tabs.Trigger>
          </Tabs.List>
          <Tabs.Content value="logs">
            <ScriptConsole logs={response.logs || []} />
            {!!response.request_updates?.length && (
              <>
                <Text as="p" size="2" weight="medium" mt="4">
                  脚本修改的本次请求
                </Text>
                <Text as="p" size="1" color="gray">
                  以下修改仅用于本次执行。
                </Text>
                <Table.Root>
                  <Table.Header>
                    <Table.Row>
                      <Table.ColumnHeaderCell>字段</Table.ColumnHeaderCell>
                      <Table.ColumnHeaderCell>执行值</Table.ColumnHeaderCell>
                    </Table.Row>
                  </Table.Header>
                  <Table.Body>
                    {response.request_updates.map((update, index) => (
                      <Table.Row key={index}>
                        <Table.Cell className="mono">{update.field}</Table.Cell>
                        <Table.Cell className="mono wrap-anywhere">
                          {update.value}
                        </Table.Cell>
                      </Table.Row>
                    ))}
                  </Table.Body>
                </Table.Root>
              </>
            )}
          </Tabs.Content>
          <Tabs.Content value="body">
            <div className="code-toolbar">
              <Text size="1" color="gray">
                {isJson
                  ? "JSON"
                  : response.body_base64
                    ? "二进制 / Base64"
                    : "Text"}
              </Text>
              <Button
                variant="ghost"
                size="1"
                color="gray"
                onClick={() => setPretty(!pretty)}
              >
                {pretty ? "查看原文" : "格式化"}
              </Button>
            </div>
            {response.truncated && (
              <Callout.Root color="amber">
                <Callout.Text>响应超过预览限制，当前内容已截断。</Callout.Text>
              </Callout.Root>
            )}
            <Editor
              value={response.body_base64 || content}
              dark={dark}
              jsonMode={isJson}
              readOnly
              height={fill ? "100%" : "300px"}
            />
          </Tabs.Content>
          <Tabs.Content value="headers">
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.ColumnHeaderCell>名称</Table.ColumnHeaderCell>
                  <Table.ColumnHeaderCell>值</Table.ColumnHeaderCell>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {response.headers.map((row, i) => (
                  <Table.Row key={i}>
                    <Table.Cell className="mono">{row.key}</Table.Cell>
                    <Table.Cell className="mono wrap-anywhere">
                      {row.value}
                    </Table.Cell>
                  </Table.Row>
                ))}
              </Table.Body>
            </Table.Root>
          </Tabs.Content>
          <Tabs.Content value="tests">
            <div className="tests-list">
              {response.tests.length === 0 ? (
                <Text color="gray" size="2">
                  在「断言」中添加测试后重新发送请求。
                </Text>
              ) : (
                response.tests.map((test) => (
                  <div className="test-result" key={test.id}>
                    {test.passed ? (
                      <Check className="test-pass" size={17} />
                    ) : (
                      <X className="test-fail" size={17} />
                    )}
                    <div>
                      <Text weight="medium" size="2">
                        {test.name}
                      </Text>
                      <Text as="p" size="1" color="gray">
                        期望 {test.expected} · 实际 {test.actual}
                      </Text>
                    </div>
                    <Badge color={test.passed ? "green" : "red"}>
                      {test.passed ? "通过" : "失败"}
                    </Badge>
                  </div>
                ))
              )}
            </div>
          </Tabs.Content>
        </Tabs.Root>
      )}
    </section>
  );
}
