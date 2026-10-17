import { errorCopy } from "../../shared/i18n/errors";
import { t, useLanguage } from "../../shared/i18n";
import { Button, TextArea, Text } from "@radix-ui/themes";
import { Upload } from "lucide-react";
import { Choice, Field } from "../../shared/ui";
import { pickFile } from "../../shared/api";
import { useWorkbench } from "../workbench/context";
export default function InterchangeFields() {
  useLanguage();
  const {
    modal,
    format,
    setFormat,
    includeSecrets,
    setIncludeSecrets,
    content,
    setContent,
    setModalError,
    draft,
  } = useWorkbench();
  return (
    <>
      <Field label={t("数据格式")}>
        <Choice
          value={format}
          onChange={setFormat}
          options={
            modal === "import"
              ? [
                  { value: "openapi", label: t("OpenAPI / Swagger JSON 或 YAML") },
                  { value: "postman", label: "Postman Collection JSON" },
                  { value: "moleapi", label: "MoleAPI JSON" },
                  { value: "curl", label: "cURL" },
                  { value: "har", label: "HAR 1.2 JSON" },
                ]
              : [
                  { value: "moleapi", label: "MoleAPI JSON" },
                  { value: "postman", label: "Postman Collection 2.1 JSON" },
                  { value: "openapi", label: "OpenAPI JSON" },
                  { value: "har", label: "HAR 1.2 JSON" },
                ]
          }
          label={t("导入导出格式")}
        />
      </Field>
      {modal === "export" && format === "har" && (
        <Text size="1" color="gray">{t("HAR 导出保存的 HTTP 请求模板，不包含实际流量、耗时或缓存；完整备份请使用 MoleAPI JSON。")}</Text>
      )}
      {modal === "import" && (
        <>
          <Button
            variant="soft"
            color="gray"
            type="button"
            onClick={async () => {
              try {
                const text = await pickFile();
                if (text !== null) setContent(text);
              } catch (error) {
                setModalError(errorCopy(error));
              }
            }}
          >
            <Upload size={15} /> {t("选择文件")} </Button>
          <Field label={t("文件内容或 cURL")}>
            <TextArea
              required
              rows={10}
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder={t("粘贴 JSON、YAML 或 cURL 命令…")}
            />
          </Field>
        </>
      )}
      {modal === "export" && (
        <>
        <Field label={t("密钥处理")}>
          <Choice
            value={includeSecrets ? "include" : "exclude"}
            onChange={(value) => setIncludeSecrets(value === "include")}
            options={[
              { value: "exclude", label: t("默认排除密钥") },
              { value: "include", label: t("包含密钥值") },
            ]}
            label={t("导出密钥")}
          />
        </Field>
        {!includeSecrets && <Text size="1" color="gray"> {t("含私密值或无法安全处理的 XML 会留空。需要完整备份时请选择包含密钥值，并妥善保管文件。")} </Text>}
        {!includeSecrets && format === "moleapi" && draft?.data.collections.some(collection=>collection.requests.some(request=>request.examples.some(example=>!!example.mock_script))) && <Text size="1" color="amber">{t("默认备份会移除 Mock 实现脚本；如需完整恢复，请选择包含密钥值。")}</Text>}
        </>
      )}
    </>
  );
}
