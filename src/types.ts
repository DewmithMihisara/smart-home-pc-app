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

export const MIN_TEMP = 16;
export const MAX_TEMP = 30;
