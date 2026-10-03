import { useState } from "react";
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
import { json } from "@codemirror/lang-json";
import { javascript } from "@codemirror/lang-javascript";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { EditorView } from "@codemirror/view";
import { oneDark } from "@codemirror/theme-one-dark";
import type { Pair } from "../types";
import { pair } from "../model";
import type { ReactNode } from "react";

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
}: {
  value: T;
  onChange: (value: T) => void;
  options: { value: T; label: string }[];
  label: string;
}) {
  return (
    <Select.Root value={value} onValueChange={(value) => onChange(value as T)}>
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
  language?: "json" | "javascript";
  label?: string;
}) {
  return (
    <CodeMirror
      className="code-editor"
      value={value}
      onChange={onChange}
      height={height}
      theme={dark ? [oneDark, accessibleDarkSyntax] : "light"}
      extensions={[
        EditorView.contentAttributes.of({
          "aria-label": label || (readOnly ? "只读代码" : "代码编辑器"),
          tabindex: "0",
        }),
        ...(language === "javascript"
          ? [javascript()]
          : jsonMode || language === "json"
            ? [json()]
            : []),
      ]}
      readOnly={readOnly}
      basicSetup={{
        lineNumbers: true,
        foldGutter: true,
        highlightActiveLine: !readOnly,
        autocompletion: !readOnly,
      }}
    />
  );
}
export function PairEditor({
  rows,
  onChange,
  secrets = false,
  keyLabel = "名称",
  valueLabel = "值",
  readLocal,
  writeLocal,
}: {
  rows: Pair[];
  onChange: (rows: Pair[]) => void;
  secrets?: boolean;
  keyLabel?: string;
  valueLabel?: string;
  readLocal?: (row: Pair) => string | undefined;
  writeLocal?: (row: Pair, value: string | undefined) => void;
}) {
  const [visible, setVisible] = useState<Set<string>>(new Set());
  const patch = (id: string, updates: Partial<Pair>) =>
    onChange(rows.map((row) => (row.id === id ? { ...row, ...updates } : row)));
  return (
    <div className={`pairs ${writeLocal ? "pairs-with-local" : ""}`}>
      <div className="pair-labels">
        <span />
        <span>{keyLabel}</span>
        <span>{valueLabel}</span>
        {writeLocal && <span>本地覆盖值</span>}
        <span />
      </div>
      {rows.map((row) => (
        <div className="pair-row" key={row.id}>
          <Checkbox
            checked={row.enabled}
            onCheckedChange={(value) =>
              patch(row.id, { enabled: value === true })
            }
            aria-label={`启用 ${row.key || keyLabel}`}
          />
          <TextField.Root
            aria-label={keyLabel}
            placeholder={keyLabel}
            value={row.key}
            onChange={(event) => patch(row.id, { key: event.target.value })}
          />
          <TextField.Root
            aria-label={valueLabel}
            placeholder={valueLabel}
            type={row.secret && !visible.has(row.id) ? "password" : "text"}
            value={row.value}
            onChange={(event) => patch(row.id, { value: event.target.value })}
          >
            {(secrets || row.secret) && (
              <TextField.Slot side="right">
                <IconButton
                  size="1"
                  variant="ghost"
                  color="gray"
                  aria-label={row.secret ? "显示或隐藏密钥" : "标记为密钥"}
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
              className="local-value-cell"
              aria-label={`本地覆盖值 ${row.key || keyLabel}`}
              placeholder="跟随共享值"
              type={row.secret && !visible.has(row.id) ? "password" : "text"}
              value={readLocal?.(row) ?? ""}
              onChange={(e) => writeLocal(row, e.target.value)}
            >
              <TextField.Slot side="right">
                <IconButton
                  size="1"
                  variant="ghost"
                  color="gray"
                  aria-label={`清除 ${row.key || keyLabel} 本地覆盖`}
                  disabled={readLocal?.(row) === undefined}
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
                checked={!!row.secret}
                onCheckedChange={(value) =>
                  patch(row.id, { secret: value === true })
                }
                aria-label={`${row.key || keyLabel} 为密钥`}
              />
            )}
            <ToolButton
              label="删除此行"
              onClick={() => onChange(rows.filter((x) => x.id !== row.id))}
            >
              <Trash2 size={14} />
            </ToolButton>
          </Flex>
        </div>
      ))}
      <Button
        variant="ghost"
        size="2"
        onClick={() => onChange([...rows, pair()])}
      >
        <Plus size={15} />
        添加一行
      </Button>
    </div>
  );
}
