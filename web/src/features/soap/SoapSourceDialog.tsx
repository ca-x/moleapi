import { useEffect, useRef, useState } from "react";
import {
  Button,
  Callout,
  Checkbox,
  Dialog,
  Flex,
  Text,
  TextField,
} from "@radix-ui/themes";
import { Plus, Trash2, Download, Upload } from "lucide-react";
import { api } from "../../shared/api";
import { id, safeMessage } from "../../shared/model";
import { Choice, Editor, Field, ToolButton } from "../../shared/ui";
import { checkSoapFiles, pickSoapFiles } from "./files";
import { useWorkbench } from "../workbench/context";
import type { SoapSchemaResult, SoapSource } from "./types";
import type { useSoapSchema } from "./useSoapSchema";
export default function SoapSourceDialog({
  open,
  onOpenChange,
  source,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  source: ReturnType<typeof useSoapSchema>;
}) {
  const state = useWorkbench();
  const [name, setName] = useState("SOAP 服务定义");
  const [files, setFiles] = useState<
    { id: string; path: string; content: string }[]
  >([]);
  const [selected, setSelected] = useState("");
  const [entry, setEntry] = useState("");
  const [url, setUrl] = useState("");
  const [verify, setVerify] = useState(true);
  const [error, setError] = useState("");
  const [pending, setPending] = useState<number | null>(null);
  const opened = useRef(open);
  const epoch = useRef(0);
  if (opened.current !== open) {
    opened.current = open;
    epoch.current++;
  }
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    if (!open) return;
    let initial: SoapSource | undefined;
    try {
      const parsed = JSON.parse(source.specification?.source || "{}");
      if (
        typeof parsed.entry_file === "string" &&
        Array.isArray(parsed.files) &&
        parsed.files.every(
          (item: { path?: unknown; content?: unknown }) =>
            typeof item.path === "string" && typeof item.content === "string",
        )
      )
        initial = parsed;
    } catch {
      /* Invalid sources are reported by Rust schema loader. */
    }
    const rows = (initial?.files || []).map((item) => ({ ...item, id: id() }));
    setFiles(rows);
    setSelected(rows[0]?.id || "");
    setEntry(initial?.entry_file || "");
    setName(source.specification?.name || "SOAP 服务定义");
    setError("");
    setUrl("");
  }, [open]);
  const busy = pending === epoch.current;
  const file = files.find((item) => item.id === selected);
  function change(patch: Partial<(typeof files)[number]>) {
    setFiles((rows) =>
      rows.map((item) => (item.id === selected ? { ...item, ...patch } : item)),
    );
  }
  async function pick() {
    if (busy) return;
    const context = source.guard();
    const generation = epoch.current;
    const current = () =>
      context() &&
      mounted.current &&
      opened.current &&
      epoch.current === generation;
    setPending(generation);
    setError("");
    try {
      const values = await pickSoapFiles();
      if (!values || !current()) return;
      const next = values.map((value) => ({ ...value, id: id() }));
      setFiles((rows) => [
        ...rows.filter((row) => !next.some((value) => value.path === row.path)),
        ...next,
      ]);
      setSelected(next[0].id);
      if (!entry)
        setEntry(
          next.find((value) => value.path.endsWith(".wsdl"))?.path ||
            next[0].path,
        );
    } catch (caught) {
      if (current()) setError(safeMessage(caught));
    } finally {
      if (mounted.current)
        setPending((value) => (value === generation ? null : value));
    }
  }
  async function validate(fromUrl = false) {
    if (busy || !state.draft) return;
    const context = source.guard();
    const generation = epoch.current;
    const current = () =>
      context() &&
      mounted.current &&
      opened.current &&
      epoch.current === generation;
    setPending(generation);
    setError("");
    try {
      if (
        !fromUrl &&
        (!files.length || !files.some((file) => file.path === entry))
      )
        throw new Error(
          "选择一个 WSDL 入口文件，并提供所引用的 XSD/WSDL 依赖。",
        );
      if (!fromUrl) checkSoapFiles(files);
      const collection = state.draft.data.collections.find((item) =>
        item.requests.some((request) => request.id === state.request?.id),
      )?.id;
      const environment = state.draft.data.active_environment_id;
      const locals = state.localVariables.values(
        state.draft,
        collection,
        environment,
      );
      const contextFields = {
        environment_id: environment,
        ...(locals.length ? { locals } : {}),
      };
      const candidate = await api<SoapSchemaResult>(
        fromUrl ? "/api/soap/import-url" : "/api/soap/import",
        "POST",
        fromUrl
          ? {
              workspace_id: state.draft.id,
              ...contextFields,
              name: name.trim() || "SOAP 服务定义",
              url,
              verify_tls: verify,
              timeout_ms: 30000,
            }
          : {
              workspace_id: state.draft.id,
              ...contextFields,
              name: name.trim() || "SOAP 服务定义",
              source: JSON.stringify({
                entry_file: entry,
                files: files.map(({ path, content }) => ({ path, content })),
              }),
            },
      );
      if (!current()) return;
      source.attach(candidate.specification, candidate.schema);
      onOpenChange(false);
    } catch (caught) {
      if (current()) setError(safeMessage(caught));
    } finally {
      if (mounted.current)
        setPending((value) => (value === generation ? null : value));
    }
  }
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content maxWidth="980px" className="grpc-source-dialog">
        <Dialog.Title>WSDL / XSD 来源</Dialog.Title>
        <Dialog.Description size="2">
          原始定义保留在工作区。相对路径与 import
          一致；引用的依赖文件需一并提供。
        </Dialog.Description>
        <Flex gap="3" mt="4" align="end" wrap="wrap">
          <Field label="WSDL 名称">
            <TextField.Root
              value={name}
              disabled={busy}
              onChange={(event) => setName(event.target.value)}
            />
          </Field>
          <Button
            variant="soft"
            color="gray"
            disabled={busy}
            onClick={() => void pick()}
          >
            <Upload size={14} />
            导入文件
          </Button>
          <Button
            variant="outline"
            color="gray"
            disabled={busy}
            onClick={() => {
              const row = {
                id: id(),
                path: "source" + (files.length + 1) + ".wsdl",
                content: "",
              };
              setFiles((rows) => [...rows, row]);
              setSelected(row.id);
              if (!entry) setEntry(row.path);
            }}
          >
            <Plus size={14} />
            添加文件
          </Button>
        </Flex>
        <Flex gap="3" mt="3" align="end" wrap="wrap">
          <Field label="WSDL URL">
            <TextField.Root
              value={url}
              disabled={busy}
              placeholder="https://service.example.com/service?wsdl"
              onChange={(event) => setUrl(event.target.value)}
            />
          </Field>
          <label className="checkbox-label">
            <Checkbox
              checked={verify}
              disabled={busy}
              onCheckedChange={(value) => setVerify(value === true)}
            />
            验证 TLS
          </label>
          <Button
            variant="soft"
            disabled={busy || !url}
            onClick={() => void validate(true)}
          >
            <Download size={14} />
            读取 URL 定义
          </Button>
        </Flex>
        {error && (
          <Callout.Root color="red" role="alert" mt="3">
            <Callout.Text>{error}</Callout.Text>
          </Callout.Root>
        )}
        <div className="grpc-source-grid">
          <div className="grpc-source-files" aria-label="WSDL / XSD 文件">
            {files.map((item) => (
              <button
                className={
                  "grpc-file-row " + (item.id === selected ? "selected" : "")
                }
                key={item.id}
                disabled={busy}
                aria-pressed={item.id === selected}
                onClick={() => setSelected(item.id)}
              >
                {item.path}
              </button>
            ))}
          </div>
          {file && (
            <div className="grpc-source-editor">
              <Flex gap="3" align="end" mb="3">
                <Field label="WSDL / XSD 相对路径">
                  <TextField.Root
                    value={file.path}
                    disabled={busy}
                    onChange={(event) => {
                      if (entry === file.path) setEntry(event.target.value);
                      change({ path: event.target.value });
                    }}
                  />
                </Field>
                <ToolButton
                  label="移除定义文件"
                  disabled={busy}
                  onClick={() => {
                    setFiles((rows) =>
                      rows.filter((item) => item.id !== file.id),
                    );
                    setSelected(
                      files.find((item) => item.id !== file.id)?.id || "",
                    );
                  }}
                >
                  <Trash2 size={14} />
                </ToolButton>
              </Flex>
              <Editor
                key={file.id}
                value={file.content}
                onChange={(content) => change({ content })}
                dark={state.dark}
                language="xml"
                readOnly={busy}
                height="360px"
                label="WSDL / XSD 文件内容"
              />
            </div>
          )}
        </div>
        <Flex gap="3" align="center" mt="3">
          <Choice
            value={files.some((file) => file.path === entry) ? entry : "none"}
            label="入口 WSDL"
            disabled={busy}
            onChange={(value) => setEntry(value === "none" ? "" : value)}
            options={[
              { value: "none", label: "选择入口 WSDL" },
              ...files
                .filter((item) => item.path.endsWith(".wsdl") && item.path)
                .map((item) => ({ value: item.path, label: item.path })),
            ]}
          />
          <Text size="1" color="gray">
            读取 URL 时不会自动下载外部 import；缺少依赖时请在文件列表中补齐。
          </Text>
        </Flex>
        <Flex gap="3" justify="end" mt="4">
          <Dialog.Close>
            <Button variant="soft" color="gray">
              取消
            </Button>
          </Dialog.Close>
          <Button
            loading={busy}
            disabled={!files.length}
            onClick={() => void validate()}
          >
            验证并使用定义
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}
