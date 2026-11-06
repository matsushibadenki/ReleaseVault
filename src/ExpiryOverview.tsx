import { useState } from "react";
import { ArrowUpRight, Search, Info, Link2 } from "lucide-react";
import { credentialName, targets } from "./guide";
import type { Sheet, T, ExpiryItem, ExpiryState } from "./types";

export function ExpiryOverview({
  snap,
  t,
  open,
}: {
  snap: Sheet;
  t: T;
  open: (item: ExpiryItem) => void;
}) {
  const [filter, setFilter] = useState("attention");
  const [project, setProject] = useState("");
  const [env, setEnv] = useState("");
  const [query, setQuery] = useState("");
  const labels: Record<ExpiryState, string> = {
    invalid: t("無効", "Invalid", "无效"),
    expired: t("期限切れ", "Expired", "已过期"),
    soon: t("30日以内", "Within 30 days", "30天内"),
    unchecked: t(
      "Apple API未照合",
      "Apple API unchecked",
      "尚未通过 Apple API 核对",
    ),
    unknown: t("期限未設定", "Expiry not set", "未设置到期日期"),
    valid: t("期限に余裕あり", "Not due soon", "未临近到期"),
  };
  const items = (snap.expiryOverview?.items ?? []).filter(
    (item) =>
      (!project ||
        String(item.source_id) === project ||
        item.projects.some((p) => String(p.id) === project)) &&
      (!env || item.scope.env === env) &&
      [
        item.def.key,
        credentialName(item.def, t),
        item.source,
        ...item.projects.map((p) => p.name),
      ]
        .join(" ")
        .toLowerCase()
        .includes(query.trim().toLowerCase()),
  );
  const rows = items.filter(
    (item) =>
      filter === "all" ||
      (filter === "attention"
        ? item.state !== "valid"
        : item.flags.includes(filter as ExpiryState)),
  );
  return (
    <section className="expiry-overview">
      <div className="expiry-filters">
        <label className="input-icon">
          <Search size={15} />
          <input
            aria-label={t(
              "期限一覧を検索",
              "Search expiry overview",
              "搜索到期列表",
            )}
            placeholder={t(
              "項目・プロジェクトを検索",
              "Search credentials or projects",
              "搜索凭据或项目",
            )}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <select
          aria-label={t(
            "プロジェクトで絞り込み",
            "Filter by project",
            "按项目筛选",
          )}
          value={project}
          onChange={(e) => setProject(e.target.value)}
        >
          <option value="">
            {t("すべてのプロジェクト", "All projects", "所有项目")}
          </option>
          {snap.projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <select
          aria-label={t(
            "環境で絞り込み",
            "Filter by environment",
            "按环境筛选",
          )}
          value={env}
          onChange={(e) => setEnv(e.target.value)}
        >
          <option value="">
            {t("すべての環境", "All environments", "所有环境")}
          </option>
          {["Development", "Staging", "Production"].map((value) => (
            <option key={value}>{value}</option>
          ))}
        </select>
      </div>
      <div
        className="expiry-tabs"
        role="group"
        aria-label={t("確認する状態", "Status to review", "要查看的状态")}
      >
        {(
          [
            "attention",
            "all",
            "invalid",
            "expired",
            "soon",
            "unchecked",
            "unknown",
            "valid",
          ] as const
        ).map((value) => {
          const count = items.filter(
            (item) =>
              value === "all" ||
              (value === "attention"
                ? item.state !== "valid"
                : item.flags.includes(value)),
          ).length;
          return (
            <button
              key={value}
              aria-pressed={filter === value}
              onClick={() => setFilter(value)}
            >
              {value === "attention"
                ? t("要確認", "Needs review", "需要检查")
                : value === "all"
                  ? t("すべて", "All", "全部")
                  : labels[value]}{" "}
              <b>{count}</b>
            </button>
          );
        })}
      </div>
      <small className="expiry-context">
        {t("基準日", "As of", "基准日期")} · {snap.expiryOverview?.as_of || "—"}
        <span
          title={t(
            "証明書・キーストアと、期限を設定した項目を表示します。共有する値はまとめて表示し、開くと共有元を優先します。",
            "Shows certificates, keystores and credentials with an expiry. Shared values are grouped and open at their source when available.",
            "显示证书、密钥库和已设置到期日期的凭据。共享值合并显示，并优先打开来源项目。",
          )}
        >
          <Info size={13} />
        </span>
      </small>
      <div className="expiry-list">
        {!rows.length && (
          <div className="empty">
            {t(
              "該当する項目はありません。",
              "No matching items.",
              "没有符合条件的项目。",
            )}
          </div>
        )}
        {rows.map((item) => (
          <button
            className="expiry-item"
            key={`${item.source_id}/${item.scope.env}/${item.def.key}`}
            onClick={() => open(item)}
          >
            <div className="expiry-title">
              <strong>{credentialName(item.def, t)}</strong>
              <span className={"badge expiry-" + item.state}>
                {labels[item.state]}
              </span>
              <ArrowUpRight size={14} />
            </div>
            <code>{item.def.key}</code>
            <div className="expiry-date">
              <b>{item.expires || labels.unknown}</b>
              {item.days !== null && (
                <span>
                  {item.days === 0
                    ? t("本日期限", "Due today", "今天到期")
                    : item.days < 0
                      ? t(
                          `期限から${-item.days}日`,
                          `${-item.days} days overdue`,
                          `已过期${-item.days}天`,
                        )
                      : t(
                          `あと${item.days}日`,
                          `${item.days} days left`,
                          `剩余${item.days}天`,
                        )}
                </span>
              )}
            </div>
            <small>
              {item.source} · {item.scope.env} · {targets[item.scope.target]}
            </small>
            {item.projects.length > 1 && (
              <small className="expiry-shared">
                <Link2 size={12} />
                {item.projects.map((p) => p.name).join(" · ")}
              </small>
            )}
            {item.flags.includes("unchecked") && item.state !== "unchecked" && (
              <small>{labels.unchecked}</small>
            )}
            {item.checked && (
              <small>
                {t("最終API照合", "Last API match", "上次 API 核对")} ·{" "}
                {new Date(item.checked).toLocaleString(
                  document.documentElement.lang,
                )}
              </small>
            )}
          </button>
        ))}
      </div>
    </section>
  );
}
