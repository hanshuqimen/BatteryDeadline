import { useI18n } from "../i18n";
import { chartPath } from "../lib";
import type { ChartPoint } from "../types";
export function PowerChart({ points }: { points: ChartPoint[] }) {
  const { t, formatNumber, time } = useI18n();
  const readings = points.filter((p) => p.power_w !== null);
  if (readings.length < 2)
    return (
      <div className="chart-empty">
        <p>{t("A clearer picture is on its way.")}</p>
        <span>
          {t(
            "The chart starts after enough stable measurements. Capacity-only estimates can take longer.",
          )}
        </span>
      </div>
    );
  const maximum = Math.max(5, ...points.flatMap((p) => [p.power_w ?? 0, p.budget_w ?? 0])) * 1.15;
  return (
    <>
      <div className="chart-legend">
        <span>
          <i className="usage-line" />
          {t("Recent usage")}
        </span>
        <span>
          <i className="budget-line" />
          {t("Required average")}
        </span>
      </div>
      <svg
        className="power-chart"
        viewBox="0 0 700 170"
        role="img"
        aria-label={t("Recent power usage from {start} to {end}. Values in watts.", {
          start: time(points[0]?.timestamp),
          end: time(points.at(-1)?.timestamp),
        })}
      >
        {[0, 0.5, 1].map((level) => (
          <g key={level}>
            <line
              x1="30"
              x2="670"
              y1={140 - level * 110}
              y2={140 - level * 110}
              className="chart-grid"
            />
            <text x="1" y={144 - level * 110}>
              {formatNumber(maximum * level, 0)}
            </text>
          </g>
        ))}
        <path d={chartPath(points, "budget_w", maximum)} className="chart-budget" />
        <path d={chartPath(points, "power_w", maximum)} className="chart-usage" />
        <text x="30" y="166">
          {time(points[0]?.timestamp)}
        </text>
        <text x="670" y="166" textAnchor="end">
          {time(points.at(-1)?.timestamp)}
        </text>
      </svg>
      <details className="chart-table">
        <summary>{t("View recent measurements")}</summary>
        <table>
          <caption>{t("Most recent measurements in watts")}</caption>
          <thead>
            <tr>
              <th>{t("Time")}</th>
              <th>{t("Usage")}</th>
              <th>{t("Required average")}</th>
            </tr>
          </thead>
          <tbody>
            {points.slice(-10).map((p) => (
              <tr key={p.timestamp}>
                <td>{time(p.timestamp)}</td>
                <td>{formatNumber(p.power_w)}</td>
                <td>{formatNumber(p.budget_w)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </>
  );
}
