// Ô nhập dùng trong Inspector: sửa tự do, chỉ ghi vào dự án khi rời ô / bấm Enter
// → mỗi lần sửa là một bước undo và không biên dịch lại theo từng phím.
import { useEffect, useState } from "react";

type NumberFieldProps = {
  label: string;
  value: number | null | undefined;
  onCommit: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
  suffix?: string;
};

export function NumberField({ label, value, onCommit, min, max, step = 1, suffix }: NumberFieldProps) {
  const [text, setText] = useState(fmt(value));
  useEffect(() => setText(fmt(value)), [value]);
  const commit = () => {
    let v = Number(text.replace(",", "."));
    if (!Number.isFinite(v)) return setText(fmt(value));
    if (min !== undefined) v = Math.max(min, v);
    if (max !== undefined) v = Math.min(max, v);
    if (v !== value) onCommit(v);
    setText(fmt(v));
  };
  return (
    <label className="field">
      <span>{label}</span>
      <span className="field-input">
        <input
          type="number"
          value={text}
          step={step}
          min={min}
          max={max}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        />
        {suffix && <em>{suffix}</em>}
      </span>
    </label>
  );
}

function fmt(v: number | null | undefined) {
  return v === null || v === undefined ? "" : String(Math.round(v * 100) / 100);
}

export function TextField(props: {
  label: string;
  value: string;
  onCommit: (v: string) => void;
  multiline?: boolean;
  placeholder?: string;
}) {
  const [text, setText] = useState(props.value);
  useEffect(() => setText(props.value), [props.value]);
  const commit = () => text !== props.value && props.onCommit(text);
  return (
    <label className="field column">
      <span>{props.label}</span>
      {props.multiline ? (
        <textarea
          rows={3}
          value={text}
          placeholder={props.placeholder}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === "Enter" && e.ctrlKey && (e.target as HTMLTextAreaElement).blur()}
        />
      ) : (
        <input
          value={text}
          placeholder={props.placeholder}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        />
      )}
    </label>
  );
}

export function SelectField<T extends string>(props: {
  label: string;
  value: T;
  options: [T, string][];
  onChange: (v: T) => void;
}) {
  return (
    <label className="field">
      <span>{props.label}</span>
      <select value={props.value} onChange={(e) => props.onChange(e.target.value as T)}>
        {props.options.map(([v, name]) => (
          <option key={v} value={v}>
            {name}
          </option>
        ))}
      </select>
    </label>
  );
}

export function ColorField(props: { label: string; value: string; onChange: (v: string) => void }) {
  const [v, setV] = useState(props.value);
  useEffect(() => setV(props.value), [props.value]);
  return (
    <label className="field">
      <span>{props.label}</span>
      <input
        type="color"
        value={v.slice(0, 7)}
        onChange={(e) => setV(e.target.value)}
        onBlur={() => v !== props.value && props.onChange(v.toUpperCase())}
      />
    </label>
  );
}
