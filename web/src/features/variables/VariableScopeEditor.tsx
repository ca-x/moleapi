import { Card, Flex, Text, TextField } from "@radix-ui/themes";
import { Link2 } from "lucide-react";
import type { Pair } from "../../shared/types";
import { PairEditor, ToolButton } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
export default function VariableScopeEditor({
  scope,
  target,
  rows,
  onChange,
}: {
  scope: "project" | "collection" | "environment";
  target: string;
  rows: Pair[];
  onChange: (rows: Pair[]) => void;
}) {
  const { localVariables } = useWorkbench();
  const entries = localVariables.entries(scope, target);
  const extra = Object.entries(entries).filter(
    ([key]) => !rows.some((row) => row.key === key),
  );
  return (
    <>
      <PairEditor
        rows={rows}
        secrets
        onChange={(next) => {
          if (localVariables.reconcile(scope, target, rows, next))
            onChange(next);
        }}
        keyLabel="变量名"
        valueLabel="共享值"
        readLocal={(row) => localVariables.read(scope, target, row.key, row.id)}
        writeLocal={(row, value) =>
          localVariables.write(scope, target, row.key, value, row.id)
        }
      />
      <Text size="1" color="gray">
        共享值保存到工作区；本地覆盖值只保存在当前浏览器或本机数据库。空字符串也可作为覆盖值，点击链接按钮恢复共享值。
      </Text>
      {extra.length > 0 && (
        <Card>
          <Text as="p" size="2" weight="medium">
            脚本提取的本地变量
          </Text>
          {extra.map(([key, value]) => (
            <Flex key={key} align="center" gap="3" mt="3">
              <Text className="mono" size="2">
                {key}
              </Text>
              <TextField.Root
                aria-label={`${key} 本地值`}
                type="password"
                value={value}
                onChange={(e) =>
                  localVariables.write(scope, target, key, e.target.value)
                }
              />
              <ToolButton
                label={`清除本地变量 ${key}`}
                onClick={() =>
                  localVariables.write(scope, target, key, undefined)
                }
              >
                <Link2 size={15} />
              </ToolButton>
            </Flex>
          ))}
        </Card>
      )}
    </>
  );
}
