import { invoke, isTauri } from "@tauri-apps/api/core";
import type { CommandName } from "./types";

export async function callBackend<T>(
  command: CommandName,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (isTauri()) return invoke<T>(command, args);
  if (import.meta.env.DEV && new URLSearchParams(window.location.search).has("simulate")) {
    const response = await fetch("/__simulation", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ command, ...args }),
      signal: AbortSignal.timeout(35_000),
    });
    const data: unknown = await response.json();
    if (!response.ok) {
      const message =
        typeof data === "object" &&
        data !== null &&
        "error" in data &&
        typeof data.error === "string"
          ? data.error
          : "The local simulator could not respond.";
      throw new Error(message);
    }
    return data as T;
  }
  throw new Error(
    "Open BatteryDeadline as a desktop app. The development preview needs the Rust simulator bridge and ?simulate.",
  );
}
