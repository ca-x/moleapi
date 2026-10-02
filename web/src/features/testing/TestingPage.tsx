import { Badge, Button, Card, Flex, Heading, Text } from "@radix-ui/themes";
import { FlaskConical } from "lucide-react";
import { Choice } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";

export default function TestingPage() {
  const state = useWorkbench();
  const { draft, runCollection, setRunCollection, runResult, runnerBusy, run } =
    state;
  if (!draft) return null;
  return (
    <div className="page-panel">
      <div className="page-heading">
        <div>
          <Heading size="5">集合测试</Heading>
          <Text size="2" color="gray">
            按顺序运行集合内的请求，并检查断言。
          </Text>
        </div>
      </div>
      <Flex gap="3" align="center">
        <Choice
          value={runCollection || draft.data.collections[0]?.id || "none"}
          onChange={setRunCollection}
          options={draft.data.collections.map((c) => ({
            value: c.id,
            label: c.name,
          }))}
          label="测试集合"
        />
        <Button
          loading={runnerBusy}
          disabled={!draft.data.collections.length}
          onClick={() => void run()}
        >
          <FlaskConical size={16} />
          运行集合
        </Button>
      </Flex>
      {runResult && (
        <>
          <Flex gap="4" align="center">
            <Badge color="green">通过 {runResult.passed}</Badge>
            <Badge color="red">失败 {runResult.failed}</Badge>
            <Text size="2" color="gray">
              {runResult.elapsed_ms} ms
            </Text>
          </Flex>
          {runResult.results.map((item) => (
            <Card key={item.request_id}>
              <Flex justify="between">
                <Text weight="medium">{item.request_name}</Text>
                {item.response && (
                  <Badge
                    color={
                      item.response.status < 400 &&
                      item.response.tests.every((t) => t.passed)
                        ? "green"
                        : "red"
                    }
                  >
                    {item.response.status}
                  </Badge>
                )}
              </Flex>
              {item.error && (
                <Text color="red" size="2">
                  {item.error}
                </Text>
              )}
              {item.response?.tests.map((test) => (
                <Text
                  as="p"
                  size="2"
                  key={test.id}
                  color={test.passed ? "green" : "red"}
                >
                  {test.passed ? "通过" : "失败"} · {test.name} · {test.actual}
                </Text>
              ))}
            </Card>
          ))}
        </>
      )}
    </div>
  );
}
