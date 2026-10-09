import { t, useLanguage } from "../../shared/i18n";
import { Button, Card, Flex, TextField } from "@radix-ui/themes";
import { Plus, Trash2 } from "lucide-react";
import { Choice, ToolButton,Editor } from "../../shared/ui";
import { id } from "../../shared/model";
import type { RequestSpec } from "../../shared/types";
export default function AssertionsEditor({
  request,
  update,
  dark=false,
}: {
  request: RequestSpec;
  update: (patch: Partial<RequestSpec>) => void;
  dark?:boolean;
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
                { value: "header", label: t("响应 Header 等于") },
                { value: "regex", label: t("正则匹配 / 捕获") },
                { value: "jsonpath", label: t("JSONPath 值等于") },
                { value: "xpath", label: t("XPath 值等于") },
                { value: "schema", label: t("JSON Schema 校验") },
              ]}
              label={t("断言类型")}
            />
            {["json","header","regex","jsonpath","xpath","schema"].includes(check.kind) && (
              <TextField.Root
                aria-label={check.kind==="json"||check.kind==="schema"?"JSON Pointer":check.kind==="jsonpath"?"JSONPath":check.kind==="xpath"?"XPath":check.kind==="regex"?t("正则表达式"):t("响应 Header 名称")}
                placeholder={check.kind==="jsonpath"?"$.data.id":check.kind==="xpath"?"string(//*[local-name()='id'])":check.kind==="regex"?"id=(\\d+)":check.kind==="header"?"Content-Type":"/data/id"}
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
            {check.kind!=="schema"&&<TextField.Root
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
            />}
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
          {check.kind==="schema"&&<Editor dark={dark} value={check.expected} onChange={expected=>update({assertions:request.assertions.map(value=>value.id===check.id?{...value,expected}:value)})} jsonMode height="180px" label={t("断言 JSON Schema")}/>}
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
