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
                ]
              : [
                  { value: "moleapi", label: "MoleAPI JSON" },
                  { value: "postman", label: "Postman Collection 2.1 JSON" },
                  { value: "openapi", label: "OpenAPI JSON" },
                ]
          }
          label={t("导入导出格式")}
        />
      </Field>
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
        </>
      )}
    </>
  );
}
