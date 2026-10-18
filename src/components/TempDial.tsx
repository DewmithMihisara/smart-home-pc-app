import { MAX_TEMP, MIN_TEMP } from "../types";
import { MinusIcon, PlusIcon } from "./Icons";

interface Props {
  value: number;
  label: string;
  disabled: boolean;
  onChange: (v: number) => void;
}

const SIZE = 196;
const R = 84;
const SWEEP = 270; // degrees of arc
const C = 2 * Math.PI * R;
const ARC = (C * SWEEP) / 360;

export function TempDial({ value, label, disabled, onChange }: Props) {
  const pct = (value - MIN_TEMP) / (MAX_TEMP - MIN_TEMP);
  const step = (d: number) => onChange(Math.min(MAX_TEMP, Math.max(MIN_TEMP, value + d)));
  const whole = Math.trunc(value);
  const half = value % 1 !== 0;

  return (
    <div className={`dial ${disabled ? "is-off" : ""}`}>
      <button className="dial-btn" aria-label="Lower temperature" disabled={disabled || value <= MIN_TEMP} onClick={() => step(-0.5)}>
        <MinusIcon />
      </button>

      <div className="dial-face">
        <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`}>
          <defs>
            <linearGradient id="arc" x1="0" y1="1" x2="1" y2="0">
              <stop offset="0%" stopColor="var(--accent-2)" />
              <stop offset="100%" stopColor="var(--accent)" />
            </linearGradient>
          </defs>
          <g transform={`rotate(135 ${SIZE / 2} ${SIZE / 2})`}>
            <circle className="dial-track" cx={SIZE / 2} cy={SIZE / 2} r={R} strokeDasharray={`${ARC} ${C}`} />
            <circle
              className="dial-value"
              cx={SIZE / 2}
              cy={SIZE / 2}
              r={R}
              stroke="url(#arc)"
              strokeDasharray={`${ARC * pct} ${C}`}
            />
          </g>
        </svg>
        <div className="dial-readout">
          <div className="dial-temp">
            {whole}
            <span className="dial-frac">{half ? ".5" : ""}</span>
            <span className="dial-unit">°C</span>
          </div>
          <div className="dial-label">{label}</div>
        </div>
      </div>

      <button className="dial-btn" aria-label="Raise temperature" disabled={disabled || value >= MAX_TEMP} onClick={() => step(0.5)}>
        <PlusIcon />
      </button>
    </div>
  );
}
