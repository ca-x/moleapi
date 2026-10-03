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
        示例：pm.test('状态为200', () =&gt; pm.response.to.have.status(200));
      </Text>
    </div>
  );
}
