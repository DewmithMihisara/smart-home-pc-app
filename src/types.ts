export type Mode = "auto" | "cool" | "dry" | "heat" | "fan";
export type FanSpeed = "auto" | "low" | "medium" | "high";

export interface AcState {
  power: boolean;
  mode: Mode;
  target: number; // °C, 16–30 in 0.5 steps
  fan: FanSpeed;
  swing: boolean;
  eco: boolean;
  turbo: boolean;
  indoor: number | null;
  outdoor: number | null;
}

export type AcPatch = Partial<Pick<AcState, "power" | "mode" | "target" | "fan" | "swing" | "eco" | "turbo">>;

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
