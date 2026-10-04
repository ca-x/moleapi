import { t, useLanguage, message } from "../../shared/i18n";
import { Button, Card, Flex, Heading, Text, TextField } from "@radix-ui/themes";
import { Plus, Save, Trash2 } from "lucide-react";
import ProjectVariables from "../variables/ProjectVariables";
import VariableScopeEditor from "../variables/VariableScopeEditor";
import { Field, ToolButton } from "../../shared/ui";
import { id } from "../../shared/model";
import { useWorkbench } from "../workbench/context";

export default function EnvironmentsPage() {
  useLanguage();
  const state = useWorkbench();
  const { draft, saving, setGuard, dirty, updateData, save, environment } =
    state;
  if (!draft) return null;
  return (
    <div className="page-panel">
      <div className="page-heading">
        <div>
          <Heading size="5">{t("环境变量")}</Heading>
          <Text size="2" color="gray"> {t("切换开发、测试与生产环境，使用 {{variable}} 引用值。", { variable: "{{variable}}" })} </Text>
        </div>
        <Button
          onClick={() => {
            const next = { id: id(), name: t("新建环境"), variables: [] };
            updateData((data) => ({
              ...data,
              environments: [...data.environments, next],
              active_environment_id: next.id,
            }));
          }}
        >
          <Plus size={16} /> {t("新建环境")} </Button>
      </div>
      <Flex gap="3" wrap="wrap">
        {draft.data.environments.map((e) => (
          <Button
            key={e.id}
            className={
              environment?.id === e.id ? "environment-selected" : undefined
            }
            aria-pressed={environment?.id === e.id}
            variant={environment?.id === e.id ? "soft" : "outline"}
            color={environment?.id === e.id ? "cyan" : "gray"}
            onClick={() =>
              updateData((data) => ({ ...data, active_environment_id: e.id }))
            }
          >
            {e.name}
          </Button>
        ))}
      </Flex>
      {environment ? (
        <Card className="environment-card">
          <Flex justify="between" align="center" gap="4">
            <Field label={t("环境名称")}>
              <TextField.Root
                value={environment.name}
                onChange={(e) =>
                  updateData((data) => ({
                    ...data,
                    environments: data.environments.map((x) =>
                      x.id === environment.id
                        ? { ...x, name: e.target.value }
                        : x,
                    ),
                  }))
                }
              />
            </Field>
            <ToolButton
              label={t("删除当前环境")}
              onClick={() =>
                setGuard({
                  title: message("删除环境"),
                  description: message("删除「{{value0}}」，保存后生效。", { value0: environment.name }),
                  action: () =>
                    updateData((data) => ({
                      ...data,
                      environments: data.environments.filter(
                        (e) => e.id !== environment.id,
                      ),
                      active_environment_id: null,
                    })),
                })
              }
            >
              <Trash2 size={16} />
            </ToolButton>
          </Flex>
          <VariableScopeEditor
            scope="environment"
            target={environment.id}
            rows={environment.variables}
            onChange={(variables) =>
              updateData((data) => ({
                ...data,
                environments: data.environments.map((e) =>
                  e.id === environment.id ? { ...e, variables } : e,
                ),
              }))
            }
          />
          <Text size="1" color="gray"> {t("勾选密钥后会隐藏值，导出默认排除密钥。工作区内的值会保存在当前数据库。")} </Text>
        </Card>
      ) : (
        <Text color="gray">{t("选择或新建一个环境。")}</Text>
      )}
      <ProjectVariables />
      <Button loading={saving} disabled={!dirty} onClick={() => void save()}>
        <Save size={16} /> {t("保存工作区")} </Button>
    </div>
  );
}
