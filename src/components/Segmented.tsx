import type { ReactNode } from "react";

export interface Option<T extends string> {
  value: T;
  label: string;
  icon?: ReactNode;
}

interface Props<T extends string> {
  options: Option<T>[];
  value: T;
  disabled?: boolean;
  onChange: (v: T) => void;
}

export function Segmented<T extends string>({ options, value, disabled, onChange }: Props<T>) {
  const idx = Math.max(0, options.findIndex((o) => o.value === value));
  return (
    <div className="seg" role="radiogroup" aria-disabled={disabled}>
      <div
        className="seg-thumb"
        style={{ width: `calc((100% - 8px) / ${options.length})`, transform: `translateX(${idx * 100}%)` }}
      />
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={`seg-item ${o.value === value ? "is-active" : ""}`}
          disabled={disabled}
          onClick={() => onChange(o.value)}
          title={o.label}
        >
          {o.icon}
          <span>{o.label}</span>
        </button>
      ))}
    </div>
  );
}
