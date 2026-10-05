import { useI18n } from "../i18n";
import { ArrowDown, ArrowUp, CheckCircle2 } from "lucide-react";

import type { Activity } from "../types";
export function ActivityList({ items }: { items: Activity[] }) {
  const { t, tr, formatNumber, time } = useI18n();
  if (!items.length)
    return (
      <p className="empty-note">
        {t("Your session activity will appear here. Every adjustment includes its reason.")}
      </p>
    );
  return (
    <ol className="activity-list">
      {items.slice(0, 15).map((item) => (
        <li key={item.id}>
          <time dateTime={item.timestamp}>{time(item.timestamp)}</time>
          <details>
            <summary>
              <CheckCircle2 size={16} aria-hidden="true" />
              <span>{tr(item.title)}</span>
              {item.before !== null && item.after !== null && (
                <span className="activity-change">
                  {item.before}{" "}
                  {item.after < item.before ? (
                    <ArrowDown size={13} aria-hidden="true" />
                  ) : (
                    <ArrowUp size={13} aria-hidden="true" />
                  )}{" "}
                  {item.after}
                  {item.kind === "refresh_rate" ? " Hz" : "%"}
                </span>
              )}
            </summary>
            <p>{tr(item.detail)}</p>
            {item.observed_saving_w !== null && (
              <p className="muted">
                {t("Observed change: {watts} W. Estimate only.", {
                  watts: formatNumber(-item.observed_saving_w),
                })}
              </p>
            )}
          </details>
        </li>
      ))}
    </ol>
  );
}
