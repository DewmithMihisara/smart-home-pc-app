import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import * as api from "../api";
import type { Config } from "../types";

interface Props {
  config: Config;
  onBack: () => void;
  onAdd: () => void;
  onChanged: () => void;
}

export function Settings({ config, onBack, onAdd, onChanged }: Props) {
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    isEnabled().then(setAutostart).catch(() => setAutostart(null));
  }, []);

  const toggleAutostart = async () => {
    try {
      if (autostart) await disable();
      else await enable();
      setAutostart(await isEnabled());
    } catch {
      /* not supported in dev builds on some platforms */
    }
  };

  const commitRename = async (id: string) => {
    if (draft.trim()) await api.renameDevice(id, draft);
    setEditing(null);
    onChanged();
  };

  return (
    <main className="panel setup">
      <header className="head">
        <div className="name">Settings</div>
        <button className="text-btn" onClick={onBack}>Done</button>
      </header>

      <div className="group-label">Air conditioners</div>
      <div className="list">
        {config.devices.map((d) => (
          <div key={d.id} className={`list-item static ${d.id === config.active ? "is-active" : ""}`}>
            <button className="radio" aria-label={`Use ${d.name}`} onClick={async () => { await api.selectDevice(d.id); onChanged(); }}>
              <span />
            </button>
            <div className="li-body">
              {editing === d.id ? (
                <input autoFocus value={draft} maxLength={24} onChange={(e) => setDraft(e.target.value)}
                  onBlur={() => commitRename(d.id)} onKeyDown={(e) => e.key === "Enter" && commitRename(d.id)} />
              ) : (
                <div className="li-title" onDoubleClick={() => { setEditing(d.id); setDraft(d.name); }}>{d.name}</div>
              )}
              <div className="li-sub">{d.ip}</div>
            </div>
            <button className="text-btn small" onClick={() => { setEditing(d.id); setDraft(d.name); }}>Rename</button>
            <button className="text-btn small danger" onClick={async () => { await api.removeDevice(d.id); onChanged(); }}>Remove</button>
          </div>
        ))}
      </div>
      <button className="secondary" onClick={onAdd}>+ Add air conditioner</button>

      <div className="spacer" />

      <button className="row-toggle" onClick={toggleAutostart} disabled={autostart === null}>
        <span>Launch at login</span>
        <span className={`switch ${autostart ? "is-on" : ""}`}><span /></span>
      </button>
      <button className="secondary danger" onClick={() => api.quit()}>Quit AC Remote</button>
    </main>
  );
}
