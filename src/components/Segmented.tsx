export interface Option<T extends string | number> {
  value: T;
  label: string;
}

interface Props<T extends string | number> {
  options: Option<T>[];
  /** `null` = nothing selected. */
  value: T | null;
  disabled?: boolean;
  label?: string;
  onChange: (v: T) => void;
}

export function Segmented<T extends string | number>({ options, value, disabled, label, onChange }: Props<T>) {
  return (
    <div className="seg" role="radiogroup" aria-label={label} aria-disabled={disabled}>
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={`seg-item ${o.value === value ? "is-active" : ""}`}
          disabled={disabled}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
