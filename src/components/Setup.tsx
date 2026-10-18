import { useState } from "react";
import * as api from "../api";
import type { DeviceInfo } from "../types";

interface Props {
  configuredIds: string[];
  onDone: () => void;
  onCancel?: () => void;
}

type Step = "scan" | "auth";

const suffix = (d: DeviceInfo) => d.name.split("_").pop() ?? d.id.slice(-4);

export function Setup({ configuredIds, onDone, onCancel }: Props) {
  const [step, setStep] = useState<Step>("scan");
  const [scanning, setScanning] = useState(false);
  const [found, setFound] = useState<DeviceInfo[] | null>(null);
  const [manualIp, setManualIp] = useState("");
  const [picked, setPicked] = useState<DeviceInfo | null>(null);

  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [manual, setManual] = useState(false);
  const [token, setToken] = useState("");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scan = async () => {
    setScanning(true);
    setError(null);
    try {
      setFound(await api.discoverDevices(manualIp.trim() || undefined));
    } catch (e) {
      setError(api.errorText(e));
    } finally {
      setScanning(false);
    }
  };

  const pick = (d: DeviceInfo) => {
    setPicked(d);
    setName(`AC ${suffix(d)}`);
    setError(null);
    setStep("auth");
  };

  const connect = async () => {
    if (!picked) return;
    setBusy(true);
    setError(null);
    try {
      await api.connectDevice({
        ip: picked.ip,
        port: picked.port,
        id: picked.id,
        name: name.trim() || `AC ${suffix(picked)}`,
        ...(manual ? { token, key } : { email, password }),
      });
      onDone();
    } catch (e) {
      setError(friendly(api.errorText(e)));
    } finally {
      setBusy(false);
    }
  };

  const canConnect = manual ? token.trim() && key.trim() : email.trim() && password;

  return (
    <main className="panel setup">
      <header className="head">
        <div>
          <div className="name">{step === "scan" ? "Add air conditioner" : "Connect"}</div>
          <div className="sub">{step === "scan" ? "Find your AC on this network" : `${picked?.name} · ${picked?.ip}`}</div>
        </div>
        {(step === "auth" || onCancel) && (
          <button className="text-btn" onClick={() => (step === "auth" ? setStep("scan") : onCancel?.())} disabled={busy}>
            {step === "auth" ? "Back" : "Cancel"}
          </button>
        )}
      </header>

      {step === "scan" && (
        <>
          <div className="list">
            {found === null && !scanning && <p className="hint">Make sure your PC is on the same Wi-Fi network as the air conditioner.</p>}
            {scanning && <p className="hint"><span className="spinner" /> Scanning…</p>}
            {found?.length === 0 && !scanning && <p className="hint">No air conditioners found. Try entering its IP address (see your router's device list).</p>}
            {!scanning &&
              found?.map((d) => (
                <button key={d.id} className="list-item" onClick={() => pick(d)}>
                  <div>
                    <div className="li-title">{d.name}</div>
                    <div className="li-sub">{d.ip} · protocol v{d.version}</div>
                  </div>
                  <span className="li-tag">{configuredIds.includes(d.id) ? "Added" : "Select"}</span>
                </button>
              ))}
          </div>
          <div className="spacer" />
          <label className="field">
            <span>IP address (optional)</span>
            <input value={manualIp} onChange={(e) => setManualIp(e.target.value)} placeholder="e.g. 192.168.1.3" />
          </label>
          {error && <p className="error">{error}</p>}
          <button className="primary" onClick={scan} disabled={scanning}>
            {scanning ? "Scanning…" : found ? "Scan again" : "Scan network"}
          </button>
        </>
      )}

      {step === "auth" && picked && (
        <>
          <label className="field">
            <span>Name</span>
            <input value={name} onChange={(e) => setName(e.target.value)} maxLength={24} />
          </label>

          {!manual ? (
            <>
              <label className="field">
                <span>SmartHome account email</span>
                <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} autoComplete="username" />
              </label>
              <label className="field">
                <span>Password</span>
                <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password"
                  onKeyDown={(e) => e.key === "Enter" && canConnect && !busy && connect()} />
              </label>
              <p className="hint small">
                Used once to fetch this AC's local key from Midea. Your password isn't saved — after this, control is fully local.
              </p>
            </>
          ) : (
            <>
              <label className="field">
                <span>Token (128 hex chars)</span>
                <input value={token} onChange={(e) => setToken(e.target.value)} spellCheck={false} />
              </label>
              <label className="field">
                <span>Key (64 hex chars)</span>
                <input value={key} onChange={(e) => setKey(e.target.value)} spellCheck={false} />
              </label>
            </>
          )}

          <button className="link-btn" onClick={() => setManual(!manual)} disabled={busy}>
            {manual ? "Sign in with SmartHome instead" : "I already have a token & key"}
          </button>
          <div className="spacer" />
          {error && <p className="error">{error}</p>}
          <button className="primary" onClick={connect} disabled={busy || !canConnect}>
            {busy ? <><span className="spinner" /> Connecting…</> : "Connect"}
          </button>
        </>
      )}
    </main>
  );
}

function friendly(msg: string): string {
  if (msg.includes("3201")) return "This account can't see that AC. Use the SmartHome account the AC is registered to.";
  if (/password|3102|3101|3106|login/i.test(msg) && msg.includes("cloud")) return `Sign-in failed: ${msg.replace(/^cloud error: /, "")}`;
  if (msg.includes("authentication")) return "The AC rejected the token/key. Double-check them.";
  if (msg.includes("no response")) return "The AC didn't respond. Is it powered and on this network?";
  return msg;
}
