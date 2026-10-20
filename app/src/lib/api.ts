import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { DeviceInfo, IpodView } from "./types";

export const listDevices = () => invoke<DeviceInfo[]>("list_devices");
export const addFolder = (path: string) => invoke<DeviceInfo>("add_folder", { path });
export const openIpod = (path: string) => invoke<IpodView>("open_ipod", { path });
export const closeIpod = () => invoke<void>("close_ipod");
export const eject = (path: string) => invoke<void>("eject", { path });
export const startupFolder = () => invoke<string | null>("startup_folder");

export const onDevicesChanged = (f: (d: DeviceInfo[]) => void): Promise<UnlistenFn> =>
  listen<DeviceInfo[]>("devices-changed", (e) => f(e.payload));

export async function pickFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false, title: "Choose the top folder of an iPod" });
  return typeof r === "string" ? r : null;
}

/** URL of a cover thumbnail at least `px` wide (served by the `art:` scheme). */
export const artUrl = (generation: number, imageId: number, px: number) =>
  convertFileSrc(`${generation}-${imageId}-${px}`, "art");
