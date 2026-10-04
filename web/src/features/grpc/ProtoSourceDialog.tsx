import type { ErrorCopy } from "../../shared/i18n/errors";
import { LocalizedError, errorCopy } from "../../shared/i18n/errors";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
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
import { Plus, Trash2, Upload } from "lucide-react";
import { api } from "../../shared/api";
import { id } from "../../shared/model";
import { Editor, Field, ToolButton } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
import { checkedProtoFiles, pickProtoFiles } from "./protoFiles";
import type { GrpcSchemaResult, ProtoSource } from "./types";
import type { useGrpcSchema } from "./useGrpcSchema";
export default function ProtoSourceDialog({
  open,
  onOpenChange,
  source,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  source: ReturnType<typeof useGrpcSchema>;
}) {
  useLanguage();
  const state = useWorkbench();
  const [files, setFiles] = useState<
    { id: string; path: string; content: string; entry: boolean }[]
  >([]);
  const [selected, setSelected] = useState<string>("");
  const [name, setName] = useState(t("gRPC 定义"));
  const [error, setError] = useState<ErrorCopy>("");
  const [busyEpoch, setBusyEpoch] = useState<number | null>(null);
  const opened = useRef(open);
  const dialogEpoch = useRef(0);
  if (opened.current !== open) {
    opened.current = open;
    dialogEpoch.current++;
  }
  const busy = busyEpoch === dialogEpoch.current;
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    if (!open) return;
    let value: ProtoSource | undefined;
    try {
      const parsed = JSON.parse(source.specification?.source || "{}");
      if (
        parsed.kind === "proto" &&
        Array.isArray(parsed.files) &&
        parsed.files.every(
          (file: { path?: unknown; content?: unknown }) =>
            typeof file.path === "string" && typeof file.content === "string",
        ) &&
        Array.isArray(parsed.entry_files)
      )
        value = parsed;
    } catch {
      /* Canonical errors are shown by the schema loader. */
    }
    const rows = (value?.files || []).map((file) => ({
      ...file,
      id: id(),
      entry: value!.entry_files.includes(file.path),
    }));
    setFiles(rows);
    setSelected(rows[0]?.id || "");
    setName(source.specification?.name || t("gRPC 定义"));
    setError("");
  }, [open]);
  const file = files.find((item) => item.id === selected);
  function change(patch: Partial<(typeof files)[number]>) {
    setFiles((rows) =>
      rows.map((item) => (item.id === selected ? { ...item, ...patch } : item)),
    );
  }
  async function pick() {
    if (busy) return;
    const scopeCurrent = source.guard();
    const originEpoch = dialogEpoch.current;
    const current = () =>
      scopeCurrent() &&
      originEpoch === dialogEpoch.current &&
      opened.current &&
      mounted.current;
    setBusyEpoch(originEpoch);
    try {
      const picked = await pickProtoFiles();
      if (!picked || !opened.current || !mounted.current || !current()) return;
      const next = picked.map((item) => ({ ...item, id: id(), entry: true }));
      setFiles((rows) => [
        ...rows.filter((row) => !next.some((item) => item.path === row.path)),
        ...next,
      ]);
      setSelected(next[0].id);
      setError("");
    } catch (caught) {
      if (opened.current && mounted.current && current())
        setError(errorCopy(caught));
    } finally {
      if (mounted.current)
        setBusyEpoch((value) => (value === originEpoch ? null : value));
    }
  }
  async function submit() {
    if (busy || !state.draft) return;
    const scopeCurrent = source.guard();
    const originEpoch = dialogEpoch.current;
    const current = () =>
      scopeCurrent() &&
      originEpoch === dialogEpoch.current &&
      opened.current &&
      mounted.current;
    setBusyEpoch(originEpoch);
    setError("");
    try {
      const entries = checkedProtoFiles(
        files.map(({ path, content }) => ({ path, content })),
      );
      const entry_files = files
        .filter((item) => item.entry)
        .map((item) => item.path);
      if (!entry_files.length) throw new LocalizedError("至少选择一个入口文件。");
      const candidate = await api<GrpcSchemaResult>(
        "/api/grpc/import",
        "POST",
        {
          workspace_id: state.draft.id,
          name: name.trim() || t("gRPC 定义"),
          source: JSON.stringify({
            kind: "proto",
            files: entries,
            entry_files,
          }),
        },
      );
      if (!current() || !opened.current || !mounted.current) return;
      source.attach(candidate.specification, candidate.schema);
      onOpenChange(false);
    } catch (caught) {
      if (current() && opened.current && mounted.current)
        setError(errorCopy(caught));
    } finally {
      if (mounted.current)
        setBusyEpoch((value) => (value === originEpoch ? null : value));
    }
  }
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content maxWidth="980px" className="grpc-source-dialog">
        <Dialog.Title>{t("Proto 服务定义")}</Dialog.Title>
        <Dialog.Description size="2"> {t("导入依赖文件并设置相对路径，使 import 与虚拟文件路径一致。入口文件定义需要调用的服务。")} </Dialog.Description>
        <Flex gap="3" align="end" mt="4">
          <Field label={t("定义名称")}>
            <TextField.Root
              value={name}
              disabled={busy}
              onChange={(event) => setName(event.target.value)}
            />
          </Field>
          <Button variant="soft" disabled={busy} onClick={() => void pick()}>
            <Upload size={15} /> {t("导入文件")} </Button>
          <Button
            variant="outline"
            color="gray"
            disabled={busy}
            onClick={() => {
              const next = {
                id: id(),
                path: `service${files.length + 1}.proto`,
                content: 'syntax = "proto3";\n',
                entry: true,
              };
              setFiles((rows) => [...rows, next]);
              setSelected(next.id);
            }}
          >
            <Plus size={15} /> {t("新建文件")} </Button>
        </Flex>
        {error && (
          <Callout.Root color="red" role="alert" mt="3">
            <Callout.Text>{translateCopy(error)}</Callout.Text>
          </Callout.Root>
        )}
        <div className="grpc-source-grid">
          <div className="grpc-source-files" aria-label={t("Proto 文件")}>
            {files.map((item) => (
              <button
                key={item.id}
                className={`grpc-file-row ${selected === item.id ? "selected" : ""}`}
                aria-pressed={selected === item.id}
                onClick={() => setSelected(item.id)}
              >
                {item.path}
                <Text size="1" color="gray">
                  {item.entry ? t("入口") : t("依赖")}
                </Text>
              </button>
            ))}
            {!files.length && (
              <Text color="gray" size="2"> {t("导入或新建 proto 文件。")} </Text>
            )}
          </div>
          {file && (
            <div className="grpc-source-editor">
              <Flex align="end" gap="3" mb="3">
                <Field label={t("相对虚拟路径")}>
                  <TextField.Root
                    value={file.path}
                    disabled={busy}
                    onChange={(event) => change({ path: event.target.value })}
                  />
                </Field>
                <label className="checkbox-label">
                  <Checkbox
                    checked={file.entry}
                    disabled={busy}
                    onCheckedChange={(value) =>
                      change({ entry: value === true })
                    }
                  /> {t("入口")} </label>
                <ToolButton
                  label={t("移除文件")}
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
                  <Trash2 size={16} />
                </ToolButton>
              </Flex>
              <Editor
                key={file.id}
                value={file.content}
                readOnly={busy}
                onChange={(content) => change({ content })}
                dark={state.dark}
                height="360px"
                language="protobuf"
                label={t("Proto 文件内容")}
              />
            </div>
          )}
        </div>
        <Flex justify="end" gap="3" mt="4">
          <Dialog.Close>
            <Button variant="soft" color="gray"> {t("取消")} </Button>
          </Dialog.Close>
          <Button
            loading={busy}
            disabled={!files.length}
            onClick={() => void submit()}
          > {t("验证并使用定义")} </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}
