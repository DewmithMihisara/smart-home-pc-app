export type Mode = "auto" | "cool" | "dry" | "heat" | "fan";
export type FanSpeed = "auto" | "low" | "medium" | "high";

export interface Capabilities {
  swingVertical: boolean;
  swingHorizontal: boolean;
  vaneVertical: boolean;
  vaneHorizontal: boolean;
  ieco: boolean;
  eco: boolean;
  turbo: boolean;
}

export interface AcState {
  power: boolean;
  mode: Mode;
  target: number; // °C, 16–30 in 0.5 steps
  fan: FanSpeed;
  swingV: boolean; // up/down swing
  swingH: boolean; // left/right swing
  vaneV: number; // fixed louver position 1–5, 0 = not set
  vaneH: number;
  ai: boolean; // AI ECO
  eco: boolean;
  turbo: boolean;
  indoor: number | null;
  outdoor: number | null;
  caps: Capabilities;
}

export type AcPatch = Partial<
  Pick<AcState, "power" | "mode" | "target" | "fan" | "swingV" | "swingH" | "vaneV" | "vaneH" | "ai" | "eco" | "turbo">
>;

export type Connection = "connecting" | "online" | "offline" | "unconfigured";

export interface Update {
  status: Exclude<Connection, "connecting">;
  state: AcState | null;
  error: string | null;
}

export interface DeviceInfo {
  ip: string;
  port: number;
  id: string;
  name: string;
  sn: string;
  version: number;
  device_type: number;
}

export interface DeviceSummary {
  id: string;
  name: string;
  ip: string;
}

export interface Config {
  devices: DeviceSummary[];
  active: string | null;
}

export const MIN_TEMP = 16;
export const MAX_TEMP = 30;
