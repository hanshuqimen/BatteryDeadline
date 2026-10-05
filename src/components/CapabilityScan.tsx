import { useI18n } from "../i18n";
import { CheckCircle2, MinusCircle } from "lucide-react";
import { actuatorLabels } from "../lib";
import type { AppSnapshot } from "../types";
export function CapabilityScan({ data }: { data: AppSnapshot }) {
  const { t, tr, formatNumber } = useI18n();
  const telemetry = data.telemetry;
  const health =
    telemetry?.full_charge_wh && telemetry.design_wh
      ? (telemetry.full_charge_wh / telemetry.design_wh) * 100
      : null;
  return (
    <details className="capability-scan">
      <summary>
        {t("Device check")}{" "}
        <span>
          {t("{count} controls available", {
            count: data.capabilities.filter((c) => c.supported).length,
          })}
        </span>
      </summary>
      <div className="scan-row">
        <span>{t("Battery readings")}</span>
        <strong>
          {telemetry?.remaining_wh !== null && telemetry?.remaining_wh !== undefined
            ? t("Capacity available")
            : telemetry?.battery_present
              ? t("Percentage only")
              : t("No battery found")}
        </strong>
        <p>{tr(telemetry?.explanation)}</p>
      </div>
      {data.capabilities.map((cap) => (
        <div className="scan-row" key={cap.kind}>
          <span>
            {cap.supported ? (
              <CheckCircle2 size={17} aria-hidden="true" />
            ) : (
              <MinusCircle size={17} aria-hidden="true" />
            )}
            {tr(actuatorLabels[cap.kind])}
          </span>
          <strong>
            {cap.supported
              ? cap.kind === "refresh_rate"
                ? `${cap.values.join(" / ")} Hz`
                : t("Available")
              : t("Unavailable")}
          </strong>
          <p>{tr(cap.explanation)}</p>
        </div>
      ))}
      <div className="scan-row">
        <span>{t("Battery health")}</span>
        <strong>
          {formatNumber(health, 0)}
          {health === null ? "" : "%"}
        </strong>
        <p>{t("Full-charge capacity compared with the battery's design capacity.")}</p>
      </div>
    </details>
  );
}
