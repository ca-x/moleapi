import { Button, TextArea } from "@radix-ui/themes";
import { Upload } from "lucide-react";
import { Choice, Field } from "../../shared/ui";
import { pickFile } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
export default function InterchangeFields() {
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
      <Field label="数据格式">
        <Choice
          value={format}
          onChange={setFormat}
          options={
            modal === "import"
              ? [
                  { value: "openapi", label: "OpenAPI / Swagger JSON 或 YAML" },
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
          label="导入导出格式"
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
                setModalError(safeMessage(error));
              }
            }}
          >
            <Upload size={15} />
            选择文件
          </Button>
          <Field label="文件内容或 cURL">
            <TextArea
              required
              rows={10}
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="粘贴 JSON、YAML 或 cURL 命令…"
            />
          </Field>
        </>
      )}
      {modal === "export" && (
        <Field label="密钥处理">
          <Choice
            value={includeSecrets ? "include" : "exclude"}
            onChange={(value) => setIncludeSecrets(value === "include")}
            options={[
              { value: "exclude", label: "默认排除密钥" },
              { value: "include", label: "包含密钥值" },
            ]}
            label="导出密钥"
          />
        </Field>
      )}
    </>
  );
}
