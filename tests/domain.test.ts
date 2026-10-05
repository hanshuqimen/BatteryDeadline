import { describe, expect, it } from "vitest";
import { chartPath, deadlineError, defaultDeadline, localInput, presetSettings } from "../src/lib";
import type { Settings } from "../src/types";

const settings: Settings = {
  reserve_percent: 10,
  preset: "balanced",
  min_brightness: 45,
  min_refresh_hz: 60,
  min_cpu_percent: 80,
  allow_brightness: true,
  allow_refresh: true,
  allow_cpu: true,
  resume_on_battery: false,
  close_to_tray: true,
  theme: "system",
  language: "system",
};

describe("deadline input", () => {
  it("preserves the local calendar date across midnight", () => {
    const date = new Date(2026, 9, 5, 23, 30);
    expect(defaultDeadline(date.toISOString())).toBe("2026-10-06T03:30");
    expect(localInput(date)).toBe("2026-10-05T23:30");
  });
  it("rejects invalid, expired and overly distant targets", () => {
    const now = new Date(2026, 9, 5, 12).getTime();
    expect(deadlineError("invalid", now)).not.toBeNull();
    expect(deadlineError("2026-10-05T11:00", now)).not.toBeNull();
    expect(deadlineError("2026-10-08T11:00", now)).not.toBeNull();
    expect(deadlineError("2026-10-05T18:30", now)).toBeNull();
  });
});
describe("explicit comfort presets", () => {
  it("keeps privacy and reserve preferences when applying a preset", () => {
    const result = presetSettings(
      { ...settings, reserve_percent: 25, resume_on_battery: true },
      "comfort",
    );
    expect(result.min_brightness).toBe(60);
    expect(result.min_cpu_percent).toBe(90);
    expect(result.reserve_percent).toBe(25);
    expect(result.resume_on_battery).toBe(true);
    expect(settings.min_brightness).toBe(45);
  });
});
describe("truthful chart rendering", () => {
  it("breaks the line at unknown power instead of connecting fake data", () => {
    const points = [0, 1, 2].map((index) => ({
      timestamp: new Date(2026, 9, 5, 12, index).toISOString(),
      power_w: index === 1 ? null : 10,
      budget_w: 8,
      percentage: 70,
    }));
    const path = chartPath(points, "power_w", 20);
    expect(path.match(/M/g)).toHaveLength(2);
    expect(path).not.toContain("L");
    expect(chartPath([], "power_w", 20)).toBe("");
  });
});
