import { Tabs, Text } from "@radix-ui/themes";
import { Editor } from "../../shared/ui";
import type { RequestSpec } from "../../shared/types";

export default function RequestScripts({
  request,
  update,
  dark,
}: {
  request: RequestSpec;
  update: (patch: Partial<RequestSpec>) => void;
  dark: boolean;
}) {
  return (
    <div className="request-scripts">
      <Text as="p" size="2" color="gray">
        脚本在隔离的 JavaScript 运行时执行，可使用 pm
        读取变量、修改请求和添加测试。
      </Text>
      {request.protocol?.kind &&
        ["sse", "websocket", "grpc"].includes(request.protocol.kind) && (
          <Text as="p" size="1" color="gray">
            实时协议执行请求前脚本，暂不执行响应后或逐条事件脚本。已有响应后脚本需清空后才能连接。
          </Text>
        )}
      <Tabs.Root defaultValue="pre">
        <Tabs.List>
          <Tabs.Trigger value="pre">请求前</Tabs.Trigger>
          <Tabs.Trigger value="post">响应后</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="pre">
          <Editor
            value={request.pre_request_script || ""}
            onChange={(pre_request_script) => update({ pre_request_script })}
            dark={dark}
            language="javascript"
            height="220px"
            label="请求前 JavaScript 脚本"
          />
        </Tabs.Content>
        <Tabs.Content value="post">
          <Editor
            value={request.post_response_script || ""}
            onChange={(post_response_script) =>
              update({ post_response_script })
            }
            dark={dark}
            language="javascript"
            height="220px"
            label="响应后 JavaScript 脚本"
          />
        </Tabs.Content>
      </Tabs.Root>
      <Text size="1" color="gray">
        {request.protocol?.kind === "grpc"
          ? "请求前示例：pm.environment.set('token', '本地值');"
          : "示例：pm.test('状态为200', () => pm.response.to.have.status(200));"}
      </Text>
    </div>
  );
}
