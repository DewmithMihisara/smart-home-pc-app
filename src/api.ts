import type { AcPatch, AcState } from "./types";

// Mock backend — replaced by Tauri commands once the Midea protocol is wired up.
let mock: AcState = {
  power: true,
  mode: "cool",
  target: 24,
  fan: "auto",
  swing: false,
  eco: false,
  turbo: false,
  indoor: 27.5,
  outdoor: 31,
};

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function getState(): Promise<AcState> {
  await delay(300);
  return { ...mock };
}

export async function setState(patch: AcPatch): Promise<AcState> {
  await delay(150);
  mock = { ...mock, ...patch };
  return { ...mock };
}
