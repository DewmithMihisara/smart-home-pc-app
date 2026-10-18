import { useEffect, useRef, useState, type PointerEvent, type WheelEvent, type KeyboardEvent } from "react";
import { MAX_TEMP, MIN_TEMP } from "../types";

interface Props {
  value: number;
  caption: string;
  disabled: boolean;
  onChange: (v: number) => void;
}

const SIZE = 232;
const C = SIZE / 2;
const R = 100;
const START = 135; // degrees, measured clockwise from +x (bottom-left)
const SWEEP = 270;
const STEP = 0.5;

const clamp = (v: number) => Math.min(MAX_TEMP, Math.max(MIN_TEMP, v));
const snap = (v: number) => clamp(Math.round(v / STEP) * STEP);
const toFraction = (v: number) => (v - MIN_TEMP) / (MAX_TEMP - MIN_TEMP);

function polar(deg: number, r = R) {
  const a = (deg * Math.PI) / 180;
  return { x: C + r * Math.cos(a), y: C + r * Math.sin(a) };
}

function arc(from: number, to: number) {
  const a = polar(from);
  const b = polar(to);
  const large = to - from > 180 ? 1 : 0;
  return `M ${a.x} ${a.y} A ${R} ${R} 0 ${large} 1 ${b.x} ${b.y}`;
}

/** Circular temperature control: drag the handle (or anywhere on the ring), scroll, or use arrow keys. */
export function TempDial({ value, caption, disabled, onChange }: Props) {
  const ref = useRef<SVGSVGElement>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const shown = drag ?? value;

  // Drop a stale drag value if the control gets disabled mid-drag.
  useEffect(() => {
    if (disabled) setDrag(null);
  }, [disabled]);

  const valueAt = (e: PointerEvent) => {
    const box = ref.current!.getBoundingClientRect();
    const x = ((e.clientX - box.left) / box.width) * SIZE - C;
    const y = ((e.clientY - box.top) / box.height) * SIZE - C;
    let rel = ((Math.atan2(y, x) * 180) / Math.PI - START + 720) % 360;
    // In the gap at the bottom: snap to whichever end is closer.
    if (rel > SWEEP) rel = rel > SWEEP + (360 - SWEEP) / 2 ? 0 : SWEEP;
    return snap(MIN_TEMP + (rel / SWEEP) * (MAX_TEMP - MIN_TEMP));
  };

  const onDown = (e: PointerEvent<SVGSVGElement>) => {
    if (disabled) return;
    const box = ref.current!.getBoundingClientRect();
    const dx = ((e.clientX - box.left) / box.width) * SIZE - C;
    const dy = ((e.clientY - box.top) / box.height) * SIZE - C;
    // Only grab near the ring, so clicks in the middle don't jump the value.
    if (Math.abs(Math.hypot(dx, dy) - R) > 26) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag(valueAt(e));
  };

  const onMove = (e: PointerEvent<SVGSVGElement>) => {
    if (drag !== null) setDrag(valueAt(e));
  };

  const onUp = () => {
    if (drag === null) return;
    if (drag !== value) onChange(drag);
    setDrag(null);
  };

  const onWheel = (e: WheelEvent) => {
    if (disabled) return;
    onChange(snap(value + (e.deltaY < 0 ? STEP : -STEP)));
  };

  const onKey = (e: KeyboardEvent) => {
    if (disabled) return;
    const d = { ArrowUp: STEP, ArrowRight: STEP, ArrowDown: -STEP, ArrowLeft: -STEP }[e.key];
    if (d) {
      e.preventDefault();
      onChange(snap(value + d));
    }
  };

  const end = START + SWEEP * toFraction(shown);
  const knob = polar(end);
  const whole = Math.trunc(shown);
  const half = shown % 1 !== 0;

  return (
    <div className={`dial ${disabled ? "is-disabled" : ""} ${drag !== null ? "is-dragging" : ""}`}>
      <svg
        ref={ref}
        width={SIZE}
        height={SIZE}
        viewBox={`0 0 ${SIZE} ${SIZE}`}
        role="slider"
        tabIndex={disabled ? -1 : 0}
        aria-label="Target temperature"
        aria-valuemin={MIN_TEMP}
        aria-valuemax={MAX_TEMP}
        aria-valuenow={shown}
        aria-disabled={disabled}
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerCancel={onUp}
        onWheel={onWheel}
        onKeyDown={onKey}
      >
        <path className="dial-track" d={arc(START, START + SWEEP)} />
        {shown > MIN_TEMP && <path className="dial-fill" d={arc(START, end)} />}
        <circle className="dial-knob" cx={knob.x} cy={knob.y} r={9} />
        <text className="dial-min" x={polar(START, R).x - 2} y={polar(START, R).y + 26} textAnchor="middle">{MIN_TEMP}</text>
        <text className="dial-max" x={polar(START + SWEEP, R).x + 2} y={polar(START + SWEEP, R).y + 26} textAnchor="middle">{MAX_TEMP}</text>
      </svg>
      <div className="dial-readout">
        <div className="dial-temp">
          {whole}
          <span className="dial-frac">{half ? ".5" : ""}</span>
          <span className="dial-unit">°</span>
        </div>
        <div className="dial-caption">{caption}</div>
      </div>
    </div>
  );
}
