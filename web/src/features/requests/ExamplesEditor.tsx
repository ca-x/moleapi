import { Badge, Card, Flex, Text, TextField } from "@radix-ui/themes";
import { Trash2 } from "lucide-react";
import { Editor, ToolButton } from "../../shared/ui";
import type { RequestSpec } from "../../shared/types";
export default function ExamplesEditor({
  request,
  update,
  dark,
}: {
  request: RequestSpec;
  update: (patch: Partial<RequestSpec>) => void;
  dark: boolean;
}) {
  return (
    <div className="example-list">
      {request.examples.length === 0 && (
        <Text size="2" color="gray">
          发送请求后，点击响应面板的「保存示例」。
        </Text>
      )}
      {request.examples.map((example) => (
        <Card key={example.id}>
          <Flex justify="between" align="center">
            <TextField.Root
              aria-label="示例名称"
              value={example.name}
              onChange={(e) =>
                update({
                  examples: request.examples.map((x) =>
                    x.id === example.id ? { ...x, name: e.target.value } : x,
                  ),
                })
              }
            />
            <Flex gap="3">
              <Badge color="gray">{example.status}</Badge>
              <ToolButton
                label="删除示例"
                onClick={() =>
                  update({
                    examples: request.examples.filter(
                      (x) => x.id !== example.id,
                    ),
                  })
                }
              >
                <Trash2 size={15} />
              </ToolButton>
            </Flex>
          </Flex>
          <Editor value={example.body} readOnly dark={dark} height="150px" />
        </Card>
      ))}
    </div>
  );
}
