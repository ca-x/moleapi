import { t, useLanguage } from "../../shared/i18n";
import { Button, Card, Flex, TextField } from "@radix-ui/themes";
import { Plus, Trash2 } from "lucide-react";
import { Choice, ToolButton } from "../../shared/ui";
import { id } from "../../shared/model";
import type { RequestSpec } from "../../shared/types";
export default function AssertionsEditor({
  request,
  update,
}: {
  request: RequestSpec;
  update: (patch: Partial<RequestSpec>) => void;
}) {
  useLanguage();
  return (
    <div className="assertion-list">
      {request.assertions.map((check) => (
        <Card key={check.id}>
          <Flex align="center" gap="3" wrap="wrap">
            <TextField.Root
              aria-label={t("断言名称")}
              value={check.name}
              onChange={(e) =>
                update({
                  assertions: request.assertions.map((x) =>
                    x.id === check.id ? { ...x, name: e.target.value } : x,
                  ),
                })
              }
            />
            <Choice
              value={check.kind}
              onChange={(kind) =>
                update({
                  assertions: request.assertions.map((x) =>
                    x.id === check.id ? { ...x, kind } : x,
                  ),
                })
              }
              options={[
                { value: "status", label: t("状态码等于") },
                { value: "duration", label: t("耗时不超过 (ms)") },
                { value: "contains", label: t("响应包含") },
                { value: "json", label: t("JSON 值等于") },
              ]}
              label={t("断言类型")}
            />
            {check.kind === "json" && (
              <TextField.Root
                aria-label="JSON Pointer"
                placeholder="/data/id"
                value={check.target}
                onChange={(e) =>
                  update({
                    assertions: request.assertions.map((x) =>
                      x.id === check.id ? { ...x, target: e.target.value } : x,
                    ),
                  })
                }
              />
            )}
            <TextField.Root
              aria-label={t("期望值")}
              placeholder={
                check.kind === "json" ? t("JSON，例如 \"ok\" 或 123") : t("期望值")
              }
              value={check.expected}
              onChange={(e) =>
                update({
                  assertions: request.assertions.map((x) =>
                    x.id === check.id ? { ...x, expected: e.target.value } : x,
                  ),
                })
              }
            />
            <ToolButton
              label={t("删除断言")}
              onClick={() =>
                update({
                  assertions: request.assertions.filter(
                    (x) => x.id !== check.id,
                  ),
                })
              }
            >
              <Trash2 size={15} />
            </ToolButton>
          </Flex>
        </Card>
      ))}
      <Button
        variant="ghost"
        onClick={() =>
          update({
            assertions: [
              ...request.assertions,
              {
                id: id(),
                name: t("HTTP 状态为 200"),
                kind: "status",
                target: "",
                expected: "200",
              },
            ],
          })
        }
      >
        <Plus size={15} /> {t("添加断言")} </Button>
    </div>
  );
}
