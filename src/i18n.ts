import { createContext, createElement, useContext, useEffect, useMemo } from "react";
import type { ReactNode } from "react";
import en from "./locales/en.json";
import zh from "./locales/zh-CN.json";
import ja from "./locales/ja.json";

export type Language = "en" | "zh-CN" | "ja";
export type MessageKey = keyof typeof en;
type Parameters = Record<string, string | number>;
type Catalog = Record<MessageKey, string>;
export const catalogs = { en, "zh-CN": zh, ja } satisfies Record<Language, Catalog>;

export function resolveLanguage(
  preference: string,
  systemLanguages: readonly string[] = [],
): Language {
  if (preference === "en" || preference === "zh-CN" || preference === "ja") return preference;
  const system = (systemLanguages[0] ?? "en").toLowerCase();
  if (system.startsWith("zh")) return "zh-CN";
  if (system.startsWith("ja")) return "ja";
  return "en";
}

const patterns: readonly [RegExp, MessageKey, readonly string[]][] = [
  [
    /^Recent usage leaves (.+) minutes of margin\. Restoring some comfort gradually\.$/,
    "Recent usage leaves {minutes} minutes of margin. Restoring some comfort gradually.",
    ["minutes"],
  ],
  [
    /^Recent usage predicts reaching your reserve (.+) minutes before the deadline\.$/,
    "Recent usage predicts reaching your reserve {minutes} minutes before the deadline.",
    ["minutes"],
  ],
  [
    /^The (.+)-minute margin is below the 8-minute safety threshold\.$/,
    "The {minutes}-minute margin is below the 8-minute safety threshold.",
    ["minutes"],
  ],
  [
    /^Target: (.+) UTC, keeping (\d+)% in reserve\.$/,
    "Target: {date} UTC, keeping {reserve}% in reserve.",
    ["date", "reserve"],
  ],
];

export function createI18n(language: Language) {
  const catalog: Catalog = catalogs[language];
  const t = (key: MessageKey, parameters: Parameters = {}): string =>
    catalog[key].replace(/\{(\w+)\}/g, (token: string, name: string) =>
      String(parameters[name] ?? token),
    );
  const tr = (source: string | null | undefined): string => {
    if (!source) return "";
    if (Object.hasOwn(catalog, source)) return t(source as MessageKey);
    const simulation = source.match(/^Synthetic event: (\w+)$/);
    if (simulation) {
      const events: Record<string, MessageKey> = {
        WorkloadSpike: "Heavy workload",
        NormalWorkload: "Normal workload",
        PlugAc: "Plug in",
        UnplugAc: "Unplug",
        MissingRate: "Power rate unavailable",
        RestoreRate: "Restore power rate",
        ManualBrightness: "Change brightness manually",
        UnsupportedDisplay: "Toggle display availability",
        ActuatorFailure: "Fail next adjustment",
        ImpossibleDeadline: "Battery below reserve",
        CrashRecovery: "Rehearse recovery",
      };
      const event = simulation[1] ?? "";
      return t("Synthetic event: {event}", { event: events[event] ? t(events[event]) : event });
    }
    for (const [pattern, key, names] of patterns) {
      const match = source.match(pattern);
      if (match)
        return t(
          key,
          Object.fromEntries(names.map((name, index) => [name, match[index + 1] ?? ""])),
        );
    }
    // Preserve native error codes/diagnostic details while translating the known explanation.
    const suffix = " This control is paused for ten minutes.";
    if (source.endsWith(suffix))
      return `${tr(source.slice(0, -suffix.length))} ${t("This control is paused for ten minutes.")}`;
    const colon = source.indexOf(": ");
    if (colon > 0 && Object.hasOwn(catalog, source.slice(0, colon)))
      return `${t(source.slice(0, colon) as MessageKey)}: ${source.slice(colon + 2)}`;
    for (const key of Object.keys(catalog) as MessageKey[]) {
      if (key.endsWith(".") && source.startsWith(`${key} (`))
        return `${t(key)}${source.slice(key.length)}`;
    }
    return source;
  };
  const formatNumber = (value: number | null | undefined, digits = 1): string =>
    value == null || !Number.isFinite(value)
      ? "—"
      : new Intl.NumberFormat(language, {
          minimumFractionDigits: digits,
          maximumFractionDigits: digits,
        }).format(value);
  const time = (value: string | null | undefined): string =>
    value
      ? new Intl.DateTimeFormat(language, { hour: "2-digit", minute: "2-digit" }).format(
          new Date(value),
        )
      : "—";
  const day = (value: string): string =>
    new Intl.DateTimeFormat(language, { month: "short", day: "numeric" }).format(new Date(value));
  return { language, t, tr, formatNumber, time, day };
}

const I18nContext = createContext(createI18n("en"));
export function I18nProvider({ language, children }: { language: Language; children: ReactNode }) {
  const value = useMemo(() => createI18n(language), [language]);
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  return createElement(I18nContext.Provider, { value }, children);
}
export function useI18n() {
  return useContext(I18nContext);
}
