import { Segmented, type Option } from "./Segmented";

interface Props {
  label: string;
  swing: boolean;
  vane: number;
  canSwing: boolean;
  canPosition: boolean;
  disabled: boolean;
  onSwing: (on: boolean) => void;
  onPosition: (pos: number) => void;
}

type Choice = "swing" | 1 | 2 | 3 | 4 | 5;

/** One louver axis: continuous swing, or one of five fixed positions. */
export function Airflow({ label, swing, vane, canSwing, canPosition, disabled, onSwing, onPosition }: Props) {
  const options: Option<Choice>[] = [
    ...(canSwing ? [{ value: "swing" as const, label: "Swing" }] : []),
    ...(canPosition ? ([1, 2, 3, 4, 5] as const).map((n) => ({ value: n, label: String(n) })) : []),
  ];
  const value: Choice | null = swing ? "swing" : vane >= 1 && vane <= 5 ? (vane as Choice) : null;

  return (
    <div className="airflow">
      <span className="airflow-label">{label}</span>
      <Segmented
        label={label}
        options={options}
        value={value}
        disabled={disabled}
        onChange={(c) => (c === "swing" ? onSwing(!swing) : onPosition(c))}
      />
    </div>
  );
}
