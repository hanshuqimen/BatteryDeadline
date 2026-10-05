import { describe, expect, it } from "vitest";
import { catalogs, createI18n, resolveLanguage } from "../src/i18n";

describe("offline language support", () => {
  it("covers every English message and preserves interpolation parameters", () => {
    const placeholders = (value: string) =>
      [...value.matchAll(/\{\w+\}/g)].map((match) => match[0]).sort();
    for (const catalog of Object.values(catalogs)) {
      expect(Object.keys(catalog).sort()).toEqual(Object.keys(catalogs.en).sort());
      for (const [key, english] of Object.entries(catalogs.en)) {
        const translated = catalog[key as keyof typeof catalog];
        expect(translated.trim().length).toBeGreaterThan(0);
        expect(placeholders(translated)).toEqual(placeholders(english));
      }
    }
  });
  it("uses a saved language, detects Chinese/Japanese Windows locales, and falls back to English", () => {
    expect(resolveLanguage("en", ["zh-CN"])).toBe("en");
    expect(resolveLanguage("system", ["zh-TW"])).toBe("zh-CN");
    expect(resolveLanguage("system", ["ja-JP"])).toBe("ja");
    expect(resolveLanguage("system", ["fr-FR"])).toBe("en");
  });
  it("translates persisted activity with parameters and retains native diagnostic codes", () => {
    const zh = createI18n("zh-CN");
    expect(zh.t("{minutes} minutes ahead of your deadline", { minutes: 12 })).toBe(
      "比截止时间多 12 分钟",
    );
    expect(
      zh.tr("Recent usage predicts reaching your reserve 9 minutes before the deadline."),
    ).toContain("9 分钟");
    expect(zh.tr("Local files could not be accessed: os error 5")).toBe(
      "无法访问本地文件: os error 5",
    );
    expect(createI18n("ja").t("Start session")).toBe("セッションを開始");
    expect(zh.tr("Synthetic event: PlugAc")).toBe("模拟事件：接通电源");
    expect(zh.tr("The brightness change could not be verified.")).toBe(
      "无法验证亮度修改是否生效。",
    );
  });
});
