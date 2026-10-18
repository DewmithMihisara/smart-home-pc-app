import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "./api";
import type { AcPatch, AcState, Config, Connection, FanSpeed, Mode } from "./types";
import { TempDial } from "./components/TempDial";
import { Segmented, type Option } from "./components/Segmented";
import { Airflow } from "./components/Airflow";
import { Setup } from "./components/Setup";
import { Settings } from "./components/Settings";
import { PowerIcon, SettingsIcon } from "./components/Icons";
import "./App.css";

const MODES: Option<Mode>[] = [
  { value: "auto", label: "Auto" },
  { value: "cool", label: "Cool" },
  { value: "dry", label: "Dry" },
  { value: "heat", label: "Heat" },
  { value: "fan", label: "Fan" },
];

const FANS: Option<FanSpeed>[] = [
  { value: "auto", label: "Auto" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

const MODE_CAPTION: Record<Mode, string> = {
  auto: "Auto",
  cool: "Cooling",
  dry: "Dry",
  heat: "Heating",
  fan: "Fan only",
};

/** Batch rapid changes (e.g. scrolling the dial) into one command. */
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
          refresh();
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
  const caps = ac?.caps;
  const active = config.devices.find((d) => d.id === config.active) ?? config.devices[0];
  const ready = conn === "online" && !!ac;
  const live = ready && on;
  const status = conn === "online" ? (on ? MODE_CAPTION[mode] : "Off") : conn === "offline" ? "Unreachable" : "Connecting";
  const room = [ac?.indoor != null && `Room ${fmt(ac.indoor)}`, ac?.outdoor != null && `Outside ${fmt(ac.outdoor)}`].filter(Boolean).join("  ·  ");

  return (
    <main className={`panel mode-${mode} ${on ? "is-on" : "is-off"}`}>
      <header className="head">
        <div className="title">
          {config.devices.length > 1 ? (
            <select className="name device-select" value={active?.id} onChange={(e) => switchDevice(e.target.value)}>
              {config.devices.map((d) => (
                <option key={d.id} value={d.id}>{d.name}</option>
              ))}
            </select>
          ) : (
            <div className="name">{active?.name ?? "Air conditioner"}</div>
          )}
          <div className={`sub status-${conn}`} title={error ?? undefined}>{status}</div>
        </div>
        <div className="head-actions">
          <button className="icon-btn" aria-label="Settings" onClick={() => setView("settings")}>
            <SettingsIcon />
          </button>
          <button className={`power ${on ? "is-on" : ""}`} aria-label={on ? "Turn off" : "Turn on"} disabled={!ready} onClick={() => update({ power: !on })}>
            <PowerIcon />
          </button>
        </div>
      </header>

      {conn === "offline" && !ac ? (
        <div className="offline">
          <p>Can&apos;t reach the air conditioner.</p>
          <p className="hint small">{error}</p>
          <button className="secondary" onClick={() => { setConn("connecting"); refresh(); }}>Try again</button>
        </div>
      ) : (
        <>
          <TempDial
            value={ac?.target ?? 24}
            caption={room || " "}
            disabled={!live || mode === "fan"}
            onChange={(target) => update({ target })}
          />

          <section className="group">
            <div className="group-label">Mode</div>
            <Segmented label="Mode" options={MODES} value={ac ? mode : null} disabled={!live} onChange={(m) => update({ mode: m })} />
          </section>

          <section className="group">
            <div className="group-label">Fan</div>
            <Segmented label="Fan" options={FANS} value={ac?.fan ?? null} disabled={!live} onChange={(fan) => update({ fan })} />
          </section>

          {caps && (caps.swingVertical || caps.vaneVertical || caps.swingHorizontal || caps.vaneHorizontal) && (
            <section className="group">
              <div className="group-label">Air direction</div>
              {(caps.swingVertical || caps.vaneVertical) && (
                <Airflow
                  label="Up · down"
                  swing={!!ac?.swingV}
                  vane={ac?.vaneV ?? 0}
                  canSwing={caps.swingVertical}
                  canPosition={caps.vaneVertical}
                  disabled={!live}
                  onSwing={(v) => update({ swingV: v })}
                  onPosition={(p) => update({ swingV: false, vaneV: p })}
                />
              )}
              {(caps.swingHorizontal || caps.vaneHorizontal) && (
                <Airflow
                  label="Left · right"
                  swing={!!ac?.swingH}
                  vane={ac?.vaneH ?? 0}
                  canSwing={caps.swingHorizontal}
                  canPosition={caps.vaneHorizontal}
                  disabled={!live}
                  onSwing={(v) => update({ swingH: v })}
                  onPosition={(p) => update({ swingH: false, vaneH: p })}
                />
              )}
            </section>
          )}

          <section className="toggles">
            {caps?.ieco && (
              <Toggle label="AI Eco" on={!!ac?.ai} disabled={!live} onClick={() => update({ ai: !ac?.ai, turbo: false })} />
            )}
            {caps?.turbo && (
              <Toggle label="Turbo" on={!!ac?.turbo} disabled={!live} onClick={() => update({ turbo: !ac?.turbo, ai: false, eco: false })} />
            )}
            {caps?.eco && (
              <Toggle label="Eco" on={!!ac?.eco} disabled={!live} onClick={() => update({ eco: !ac?.eco, turbo: false })} />
            )}
          </section>
        </>
      )}
    </main>
  );
}

function Toggle(props: { label: string; on: boolean; disabled: boolean; onClick: () => void }) {
  return (
    <button className={`toggle ${props.on ? "is-on" : ""}`} disabled={props.disabled} onClick={props.onClick} aria-pressed={props.on}>
      <span className="toggle-dot" />
      {props.label}
    </button>
  );
}

export default App;
