import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AcPatch, AcState, Config, DeviceInfo, Update } from "./types";

export const getConfig = () => invoke<Config>("get_config");
export const getState = () => invoke<AcState>("get_state");
export const setState = (patch: AcPatch) => invoke<AcState>("set_state", { patch });

export const discoverDevices = (ip?: string) => invoke<DeviceInfo[]>("discover_devices", { ip: ip || null });

export interface ConnectArgs {
  ip: string;
  port: number;
  id: string;
  name: string;
  email?: string;
  password?: string;
  token?: string;
  key?: string;
}
export const connectDevice = (args: ConnectArgs) => invoke<AcState>("connect_device", { args });

export const selectDevice = (id: string) => invoke<void>("select_device", { id });
export const renameDevice = (id: string, name: string) => invoke<void>("rename_device", { id, name });
export const removeDevice = (id: string) => invoke<void>("remove_device", { id });
export const quit = () => invoke<void>("quit");

export const onUpdate = (cb: (u: Update) => void): Promise<UnlistenFn> =>
  listen<Update>("ac-update", (e) => cb(e.payload));

/** Commands reject with the Rust error's message string. */
export const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));
