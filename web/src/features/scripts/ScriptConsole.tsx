import { Badge, Flex, Text } from "@radix-ui/themes";
import type { ScriptLog } from "../../shared/types";
export default function ScriptConsole({ logs }: { logs: ScriptLog[] }) {
  return (
    <div className="script-console" role="log" aria-label="脚本控制台">
      {logs.length === 0 ? (
        <Text color="gray" size="2">
          当前请求没有脚本输出。
        </Text>
      ) : (
        logs.map((log, index) => (
          <Flex key={index} gap="3" align="start">
            <Badge
              color={
                log.level === "error"
                  ? "red"
                  : log.level === "warn"
                    ? "amber"
                    : "gray"
              }
            >
              {log.level}
            </Badge>
            <Text size="2" className="mono wrap-anywhere">
              {log.message}
            </Text>
          </Flex>
        ))
      )}
    </div>
  );
}
