import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "./api";
import type { AcPatch, AcState, Config, Connection, FanSpeed, Mode } from "./types";
import { TempDial } from "./components/TempDial";
import { Segmented, type Option } from "./components/Segmented";
import { Setup } from "./components/Setup";
import { Settings } from "./components/Settings";
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

/** Batch rapid changes (e.g. several +/- clicks) into one command. */
const SEND_DELAY = 350;
/** Ignore polled state for a moment after a local change so the UI doesn't jump back. */
const QUIET_AFTER_CHANGE = 2500;

const fmt = (t: number | null) => (t == null ? "–" : `${Math.round(t * 10) / 10}°`);

type View = "loading" | "setup" | "main" | "settings";

function App() {
  const [view, setView] = useState<View>("loading");
  const [config, setConfig] = useState<Config>({ devices: [], active: null });
  const [ac, setAc] = useState<AcState | null>(null);
  const [conn, setConn] = useState<Connection>("connecting");
  const [error, setError] = useState<string | null>(null);

  const pending = useRef<AcPatch>({});
  const timer = useRef<number | undefined>(undefined);
  const lastLocal = useRef(0);

  const refresh = useCallback(() => {
    api
      .getState()
      .then((s) => {
        setConn("online");
        setError(null);
        if (Date.now() - lastLocal.current > QUIET_AFTER_CHANGE) setAc(s);
      })
      .catch((e) => {
        setConn("offline");
        setError(api.errorText(e));
      });
  }, []);

  const loadConfig = useCallback(async () => {
    const cfg = await api.getConfig();
    setConfig(cfg);
    return cfg;
  }, []);

  // Initial load + backend events.
  useEffect(() => {
    loadConfig().then((cfg) => {
      if (cfg.devices.length === 0) {
        setView("setup");
      } else {
        setView("main");
        refresh();
      }
    });
    const unlisten = api.onUpdate((u) => {
      setConn(u.status);
      setError(u.error);
      if (u.state && Date.now() - lastLocal.current > QUIET_AFTER_CHANGE) setAc(u.state);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [loadConfig, refresh]);

  // Refresh when the popup opens; Esc hides it.
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) refresh();
    });
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && win.hide();
    window.addEventListener("keydown", onKey);
    return () => {
      unlisten.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, [refresh]);

  const update = (patch: AcPatch) => {
    if (!ac) return;
    setAc({ ...ac, ...patch }); // optimistic
    lastLocal.current = Date.now();
    pending.current = { ...pending.current, ...patch };
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      const p = pending.current;
      pending.current = {};
      lastLocal.current = Date.now();
      api
        .setState(p)
        .then((s) => {
          if (Object.keys(pending.current).length === 0) setAc(s);
        })
        .catch(() => refresh());
    }, SEND_DELAY);
  };

  const switchDevice = async (id: string) => {
    await api.selectDevice(id);
    await loadConfig();
    setAc(null);
    setConn("connecting");
    refresh();
  };

  if (view === "loading") return <main className="panel" />;

  if (view === "setup")
    return (
      <Setup
        configuredIds={config.devices.map((d) => d.id)}
        onCancel={config.devices.length ? () => setView("main") : undefined}
        onDone={async () => {
          await loadConfig();
          setView("main");
        }}
      />
    );

  if (view === "settings")
    return (
      <Settings
        config={config}
        onBack={() => setView("main")}
        onAdd={() => setView("setup")}
        onChanged={async () => {
          const cfg = await loadConfig();
          if (cfg.devices.length === 0) setView("setup");
          setAc(null);
          setConn("connecting");
          refresh();
        }}
      />
    );

  const on = !!ac?.power;
  const mode = ac?.mode ?? "cool";
  const active = config.devices.find((d) => d.id === config.active) ?? config.devices[0];
  const ready = conn === "online" && !!ac;
  const status = conn === "online" ? (on ? "Running" : "Standby") : conn === "offline" ? "Can't reach AC" : "Connecting…";

  return (
    <main className={`panel mode-${mode} ${on ? "" : "is-off"}`}>
      <header className="head">
        <div className="title">
          <span className={`dot dot-${conn}`} />
          <div>
            {config.devices.length > 1 ? (
              <select className="name device-select" value={active?.id} onChange={(e) => switchDevice(e.target.value)}>
                {config.devices.map((d) => (
                  <option key={d.id} value={d.id}>{d.name}</option>
                ))}
              </select>
            ) : (
              <div className="name">{active?.name ?? "Air conditioner"}</div>
            )}
            <div className="sub" title={error ?? undefined}>{status}</div>
          </div>
        </div>
        <div className="head-actions">
          <button className="icon-btn" aria-label="Settings" onClick={() => setView("settings")}>
            <GearIcon />
          </button>
          <button
            className={`power ${on ? "is-on" : ""}`}
            aria-label={on ? "Turn off" : "Turn on"}
            disabled={!ready}
            onClick={() => update({ power: !on })}
          >
            <PowerIcon width={20} height={20} />
          </button>
        </div>
      </header>

      {conn === "offline" && !ac ? (
        <div className="offline">
          <p>Couldn't reach the air conditioner.</p>
          <p className="hint small">{error}</p>
          <button className="secondary" onClick={() => { setConn("connecting"); refresh(); }}>Try again</button>
        </div>
      ) : (
        <>
          <TempDial
            value={ac?.target ?? 24}
            label={!ac ? "…" : on ? MODE_LABEL[mode] : "Off"}
            disabled={!ready || !on || mode === "fan"}
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
            <Segmented options={MODES} value={mode} disabled={!ready || !on} onChange={(m) => update({ mode: m })} />
          </section>

          <section className="group">
            <div className="group-label">Fan</div>
            <Segmented options={FANS} value={ac?.fan ?? "auto"} disabled={!ready || !on} onChange={(fan) => update({ fan })} />
          </section>

          <section className="chips">
            <Chip icon={<SwingIcon />} label="Swing" active={!!ac?.swing} disabled={!ready || !on} onClick={() => update({ swing: !ac?.swing })} />
            <Chip icon={<LeafIcon />} label="Eco" active={!!ac?.eco} disabled={!ready || !on} onClick={() => update({ eco: !ac?.eco, turbo: false })} />
            <Chip icon={<BoltIcon />} label="Turbo" active={!!ac?.turbo} disabled={!ready || !on} onClick={() => update({ turbo: !ac?.turbo, eco: false })} />
          </section>
        </>
      )}
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
