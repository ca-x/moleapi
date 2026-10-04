import { t, useLanguage } from "../../shared/i18n";
import { useState } from "react";
import { Card, Heading, Tabs, Text } from "@radix-ui/themes";
import { Choice } from "../../shared/ui";
import VariableScopeEditor from "./VariableScopeEditor";
import { useWorkbench } from "../workbench/context";
export default function ProjectVariables() {
  useLanguage();
  const { draft, updateData } = useWorkbench();
  const [collectionId, setCollectionId] = useState("");
  if (!draft) return null;
  const collection =
    draft.data.collections.find((c) => c.id === collectionId) ||
    draft.data.collections[0];
  return (
    <Card className="scope-variables-card">
      <Heading size="3" mb="3"> {t("项目与集合变量")} </Heading>
      <Tabs.Root defaultValue="project">
        <Tabs.List>
          <Tabs.Trigger value="project">{t("项目全局")}</Tabs.Trigger>
          <Tabs.Trigger value="collection">{t("集合变量")}</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="project">
          <VariableScopeEditor
            scope="project"
            target=""
            rows={draft.data.global_variables || []}
            onChange={(global_variables) =>
              updateData((data) => ({ ...data, global_variables }))
            }
          />
        </Tabs.Content>
        <Tabs.Content value="collection">
          {collection ? (
            <>
              <Choice
                label={t("变量所属集合")}
                value={collection.id}
                onChange={setCollectionId}
                options={draft.data.collections.map((c) => ({
                  value: c.id,
                  label: c.name,
                }))}
              />
              <VariableScopeEditor
                scope="collection"
                target={collection.id}
                rows={collection.variables || []}
                onChange={(variables) =>
                  updateData((data) => ({
                    ...data,
                    collections: data.collections.map((c) =>
                      c.id === collection.id ? { ...c, variables } : c,
                    ),
                  }))
                }
              />
            </>
          ) : (
            <Text color="gray">{t("先创建一个集合。")}</Text>
          )}
        </Tabs.Content>
      </Tabs.Root>
      <Text as="p" color="gray" size="1" mt="3"> {t("临时执行值 > 测试数据 > 环境 > 集合 > 项目全局。团队级变量将由团队资源模块提供。")} </Text>
    </Card>
  );
}
