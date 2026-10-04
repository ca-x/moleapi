import { search } from "@codemirror/search";
import { EditorState, Compartment } from "@codemirror/state";
import codeMirrorZhCN from "../i18n/codemirror-zh-CN.json";
import { t, useLanguage } from "../i18n";
import { useState, useMemo, useRef, useEffect, useCallback } from "react";
import {
  Button,
  Checkbox,
  Flex,
  IconButton,
  TextField,
  Tooltip,
  Select,
  Text,
} from "@radix-ui/themes";
import { Plus, Trash2, Eye, EyeOff, Link2 } from "lucide-react";
import CodeMirror from "@uiw/react-codemirror";
import type { ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { protobuf } from "@codemirror/legacy-modes/mode/protobuf";
import { json } from "@codemirror/lang-json";
import { xml } from "@codemirror/lang-xml";
import { javascript } from "@codemirror/lang-javascript";
import {
  HighlightStyle,
  syntaxHighlighting,
  StreamLanguage,
} from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { EditorView } from "@codemirror/view";
import { oneDark } from "@codemirror/theme-one-dark";
import type { Pair } from "../types";
import { pair } from "../model";
import type { ReactNode } from "react";

const persistentSearch = search();

const accessibleDarkSyntax = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.propertyName, class: "cm-property-accessible" },
  ]),
);

export function ToolButton({
  label,
  children,
  onClick,
  disabled,
  expanded,
  controls,
  className,
}: {
  label: string;
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  expanded?: boolean;
  controls?: string;
  className?: string;
}) {
  useLanguage();
  return (
    <Tooltip content={label} delayDuration={600}>
      <IconButton
        aria-label={label}
        aria-expanded={expanded}
        aria-controls={controls}
        className={className}
        variant="ghost"
        color="gray"
        onClick={onClick}
        disabled={disabled}
      >
        {children}
      </IconButton>
    </Tooltip>
  );
}
export function Field({
  label,
  children,
  hint,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
}) {
  useLanguage();
  return (
    <label className="field">
      <Text as="span" size="2" weight="medium">
        {label}
      </Text>
      {children}
      {hint && (
        <Text as="span" size="1" color="gray">
          {hint}
        </Text>
      )}
    </label>
  );
}
export function Choice<T extends string>({
  value,
  onChange,
  options,
  label,
  disabled = false,
}: {
  value: T;
  onChange: (value: T) => void;
  options: { value: T; label: string }[];
  label: string;
  disabled?: boolean;
}) {
  useLanguage();
  return (
    <Select.Root
      value={value}
      disabled={disabled}
      onValueChange={(value) => onChange(value as T)}
    >
      <Select.Trigger aria-label={label} />
      <Select.Content>
        {options.map((item) => (
          <Select.Item key={item.value} value={item.value}>
            {item.label}
          </Select.Item>
        ))}
      </Select.Content>
    </Select.Root>
  );
}
export function Editor({
  value,
  onChange,
  dark,
  readOnly = false,
  jsonMode = false,
  height = "240px",
  language,
  label,
}: {
  value: string;
  onChange?: (value: string) => void;
  dark: boolean;
  readOnly?: boolean;
  jsonMode?: boolean;
  height?: string;
  language?: "json" | "javascript" | "protobuf" | "xml";
  label?: string;
}) {
  const { language: interfaceLanguage } = useLanguage();
  const editor = useRef<ReactCodeMirrorRef>(null);
  const phrases = useMemo(() => new Compartment(), []);
  const attributes = useMemo(() => new Compartment(), []);
  const latestChange = useRef(onChange);
  latestChange.current = onChange;
  const change = useCallback((value: string) => latestChange.current?.(value), []);
  const extensions = useMemo(() => [
    persistentSearch,
    phrases.of(EditorState.phrases.of(interfaceLanguage === "zh-CN" ? codeMirrorZhCN : {})),
    attributes.of(EditorView.contentAttributes.of({
      "aria-label": label || (readOnly ? t("只读代码") : t("代码编辑器")), tabindex: "0",
    })),
    ...(language === "xml" ? [xml()] : language === "protobuf"
      ? [StreamLanguage.define(protobuf)] : language === "javascript"
        ? [javascript()] : jsonMode || language === "json" ? [json()] : []),
  // Locale and accessible labels are reconfigured in their own compartments.
  // Keep the full editor configuration stable while those options change.
  ], [phrases, attributes, language, jsonMode, readOnly]);
  const setup = useMemo(() => ({ lineNumbers: true, foldGutter: true,
    highlightActiveLine: !readOnly, autocompletion: !readOnly }), [readOnly]);
  const darkTheme = useMemo(() => [oneDark, accessibleDarkSyntax], []);
  useEffect(() => {
    editor.current?.view?.dispatch({ effects: [
      phrases.reconfigure(EditorState.phrases.of(interfaceLanguage === "zh-CN" ? codeMirrorZhCN : {})),
      attributes.reconfigure(EditorView.contentAttributes.of({
        "aria-label": label || (readOnly ? t("只读代码") : t("代码编辑器")), tabindex: "0",
      })),
    ] });
  }, [interfaceLanguage, label, readOnly, phrases, attributes]);
  return (
    <CodeMirror
      ref={editor}
      className="code-editor"
      value={value}
      onChange={change}
      height={height}
      theme={dark ? darkTheme : "light"}
      extensions={extensions}
      readOnly={readOnly}
      basicSetup={setup}
    />
  );
}
export function PairEditor({
  rows,
  onChange,
  secrets = false,
  disabled = false,
  keyLabel = t("名称"),
  valueLabel = t("值"),
  readLocal,
  writeLocal,
}: {
  rows: Pair[];
  onChange: (rows: Pair[]) => void;
  secrets?: boolean;
  disabled?: boolean;
  keyLabel?: string;
  valueLabel?: string;
  readLocal?: (row: Pair) => string | undefined;
  writeLocal?: (row: Pair, value: string | undefined) => void;
}) {
  useLanguage();
  const [visible, setVisible] = useState<Set<string>>(new Set());
  const patch = (id: string, updates: Partial<Pair>) =>
    onChange(rows.map((row) => (row.id === id ? { ...row, ...updates } : row)));
  return (
    <div className={`pairs ${writeLocal ? "pairs-with-local" : ""}`}>
      <div className="pair-labels">
        <span />
        <span>{keyLabel}</span>
        <span>{valueLabel}</span>
        {writeLocal && <span>{t("本地覆盖值")}</span>}
        <span />
      </div>
      {rows.map((row) => (
        <div className="pair-row" key={row.id}>
          <Checkbox
            disabled={disabled}
            checked={row.enabled}
            onCheckedChange={(value) =>
              patch(row.id, { enabled: value === true })
            }
            aria-label={t("启用 {{value0}}", { value0: row.key || keyLabel })}
          />
          <TextField.Root
            disabled={disabled}
            aria-label={keyLabel}
            placeholder={keyLabel}
            value={row.key}
            onChange={(event) => patch(row.id, { key: event.target.value })}
          />
          <TextField.Root
            disabled={disabled}
            aria-label={valueLabel}
            placeholder={valueLabel}
            type={row.secret && !visible.has(row.id) ? "password" : "text"}
            value={row.value}
            onChange={(event) => patch(row.id, { value: event.target.value })}
          >
            {(secrets || row.secret) && (
              <TextField.Slot side="right">
                <IconButton
                  disabled={disabled}
                  size="1"
                  variant="ghost"
                  color="gray"
                  aria-label={row.secret ? t("显示或隐藏密钥") : t("标记为密钥")}
                  onClick={() => {
                    if (!row.secret) return patch(row.id, { secret: true });
                    setVisible((prev) => {
                      const next = new Set(prev);
                      if (next.has(row.id)) next.delete(row.id);
                      else next.add(row.id);
                      return next;
                    });
                  }}
                >
                  {row.secret && !visible.has(row.id) ? (
                    <EyeOff size={14} />
                  ) : (
                    <Eye size={14} />
                  )}
                </IconButton>
              </TextField.Slot>
            )}
          </TextField.Root>
          {writeLocal && (
            <TextField.Root
            disabled={disabled}
              className="local-value-cell"
              aria-label={t("本地覆盖值 {{value0}}", { value0: row.key || keyLabel })}
              placeholder={t("跟随共享值")}
              type={row.secret && !visible.has(row.id) ? "password" : "text"}
              value={readLocal?.(row) ?? ""}
              onChange={(e) => writeLocal(row, e.target.value)}
            >
              <TextField.Slot side="right">
                <IconButton
                  size="1"
                  variant="ghost"
                  color="gray"
                  aria-label={t("清除 {{value0}} 本地覆盖", { value0: row.key || keyLabel })}
                  disabled={disabled || readLocal?.(row) === undefined}
                  onClick={() => writeLocal(row, undefined)}
                >
                  <Link2 size={14} />
                </IconButton>
              </TextField.Slot>
            </TextField.Root>
          )}
          <Flex align="center" gap="2">
            {secrets && (
              <Checkbox
            disabled={disabled}
                checked={!!row.secret}
                onCheckedChange={(value) =>
                  patch(row.id, { secret: value === true })
                }
                aria-label={t("{{value0}} 为密钥", { value0: row.key || keyLabel })}
              />
            )}
            <ToolButton
              disabled={disabled}
              label={t("删除此行")}
              onClick={() => onChange(rows.filter((x) => x.id !== row.id))}
            >
              <Trash2 size={14} />
            </ToolButton>
          </Flex>
        </div>
      ))}
      <Button
        disabled={disabled}
        variant="ghost"
        size="2"
        onClick={() => onChange([...rows, pair()])}
      >
        <Plus size={15} /> {t("添加一行")} </Button>
    </div>
  );
}
