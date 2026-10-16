import { invoke } from "@tauri-apps/api/core";

export type NeverWord = { word: string; source: "manual" | "undo" };

export type Config = {
  enabled: boolean;
  autostart: boolean;
  journal: boolean;
  learn: boolean;
  features: { layout: boolean; typos: boolean; replace: boolean };
  signals: { hint: boolean; blink: boolean; sound: boolean; cursor: boolean };
  hotkeys: { convert: string; toggle: string };
  replacements: Record<string, string>;
  never: NeverWord[];
  processes: { off: string[]; code: string[] };
};

export type ConfigError = { field: string; message: string };

export type FlyoutState = {
  status: "working" | "disabled" | "silent";
  reason: "process" | "password" | "fullscreen" | "elevated" | "layout" | "noruen" | null;
  app: { exe: string; name: string } | null;
  count: number;
  ru: boolean;
  config: Config;
};

export type Proc = { exe: string; title: string };
export type About = { version: string; licenses: { name: string; source: string; license: string }[] };

export const api = {
  flyoutState: () => invoke<FlyoutState>("flyout_state"),
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (config: Config) => invoke<Config>("set_config", { config }),
  removeNever: (word: string) => invoke<Config>("remove_never", { word }),
  clearNever: () => invoke<Config>("clear_never"),
  getWords: () => invoke<string[]>("get_words"),
  addWord: (word: string) => invoke<string[]>("add_word", { word }),
  removeWord: (word: string) => invoke<string[]>("remove_word", { word }),
  listProcesses: () => invoke<Proc[]>("list_processes"),
  about: () => invoke<About>("about"),
  openSettings: () => invoke<void>("open_settings_cmd"),
};

export const errorsOf = (e: unknown): ConfigError[] =>
  Array.isArray(e) ? (e as ConfigError[]) : [{ field: "file", message: String(e) }];

export const plural = (n: number, one: string, few: string, many: string) => {
  const d = n % 10, h = n % 100;
  if (d === 1 && h !== 11) return one;
  if (d >= 2 && d <= 4 && (h < 12 || h > 14)) return few;
  return many;
};
