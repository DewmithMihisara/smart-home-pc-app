import { useEffect, useState } from "react";
import * as api from "./api";
import type { AcPatch, AcState, Connection, FanSpeed, Mode } from "./types";
import { TempDial } from "./components/TempDial";
import { Segmented, type Option } from "./components/Segmented";
import {
  AutoIcon,
  BoltIcon,
  CloudIcon,
  DropIcon,
  FanIcon,
  GearIcon,
  HomeIcon,
  LeafIcon,
  PowerIcon,
  SnowIcon,
  SunIcon,
  SwingIcon,
} from "./components/Icons";
import "./App.css";

const MODES: Option<Mode>[] = [
  { value: "auto", label: "Auto", icon: <AutoIcon /> },
  { value: "cool", label: "Cool", icon: <SnowIcon /> },
  { value: "dry", label: "Dry", icon: <DropIcon /> },
  { value: "heat", label: "Heat", icon: <SunIcon /> },
  { value: "fan", label: "Fan", icon: <FanIcon /> },
];

const FANS: Option<FanSpeed>[] = [
  { value: "auto", label: "Auto" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Med" },
  { value: "high", label: "High" },
];

const MODE_LABEL: Record<Mode, string> = {
  auto: "Auto",
  cool: "Cooling to",
  dry: "Drying at",
  heat: "Heating to",
  fan: "Fan only",
};

const fmt = (t: number | null) => (t == null ? "–" : `${Math.round(t * 10) / 10}°`);

function App() {
  const [ac, setAc] = useState<AcState | null>(null);
  const [conn, setConn] = useState<Connection>("connecting");

  useEffect(() => {
    api
      .getState()
      .then((s) => {
        setAc(s);
        setConn("online");
      })
      .catch(() => setConn("offline"));
  }, []);

  const update = (patch: AcPatch) => {
    if (!ac) return;
    setAc({ ...ac, ...patch }); // optimistic
    api.setState(patch).then(setAc).catch(() => setConn("offline"));
  };

  const on = !!ac?.power;
  const mode = ac?.mode ?? "cool";

  return (
    <main className={`panel mode-${mode} ${on ? "" : "is-off"}`}>
      <header className="head">
        <div className="title">
          <span className={`dot dot-${conn}`} />
          <div>
            <div className="name">Living Room AC</div>
            <div className="sub">{conn === "online" ? (on ? "Running" : "Standby") : conn}</div>
          </div>
        </div>
        <div className="head-actions">
          <button className="icon-btn" aria-label="Settings">
            <GearIcon />
          </button>
          <button
            className={`power ${on ? "is-on" : ""}`}
            aria-label={on ? "Turn off" : "Turn on"}
            disabled={!ac}
            onClick={() => update({ power: !on })}
          >
            <PowerIcon width={20} height={20} />
          </button>
        </div>
      </header>

      <TempDial
        value={ac?.target ?? 24}
        label={on ? MODE_LABEL[mode] : "Off"}
        disabled={!on || mode === "fan"}
        onChange={(target) => update({ target })}
      />

      <section className="climate">
        <span>
          <HomeIcon width={14} height={14} /> Indoor {fmt(ac?.indoor ?? null)}
        </span>
        <span>
          <CloudIcon width={14} height={14} /> Outdoor {fmt(ac?.outdoor ?? null)}
        </span>
      </section>

      <section className="group">
        <div className="group-label">Mode</div>
        <Segmented options={MODES} value={mode} disabled={!on} onChange={(m) => update({ mode: m })} />
      </section>

      <section className="group">
        <div className="group-label">Fan</div>
        <Segmented options={FANS} value={ac?.fan ?? "auto"} disabled={!on} onChange={(fan) => update({ fan })} />
      </section>

      <section className="chips">
        <Chip icon={<SwingIcon />} label="Swing" active={!!ac?.swing} disabled={!on} onClick={() => update({ swing: !ac?.swing })} />
        <Chip icon={<LeafIcon />} label="Eco" active={!!ac?.eco} disabled={!on} onClick={() => update({ eco: !ac?.eco, turbo: false })} />
        <Chip icon={<BoltIcon />} label="Turbo" active={!!ac?.turbo} disabled={!on} onClick={() => update({ turbo: !ac?.turbo, eco: false })} />
      </section>
    </main>
  );
}

function Chip(props: { icon: React.ReactNode; label: string; active: boolean; disabled: boolean; onClick: () => void }) {
  return (
    <button className={`chip ${props.active ? "is-active" : ""}`} disabled={props.disabled} onClick={props.onClick} aria-pressed={props.active}>
      {props.icon}
      {props.label}
    </button>
  );
}

export default App;
