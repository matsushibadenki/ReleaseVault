import { useState } from "react";
import { ExternalLink, RefreshCw, Info } from "lucide-react";
import type { Sheet, FormValues, T } from "./types";

export function ActionsReport({
  snap,
  t,
  busy,
  action,
}: {
  snap: Sheet;
  t: T;
  busy: boolean;
  action: (op: string, values: FormValues) => void;
}) {
  const connection = snap.githubApp;
  const repos =
    connection?.repos.filter((r) =>
      ["read", "write"].includes(r.grants.actions ?? "none"),
    ) ?? [];
  const [repo, choose] = useState(
    snap.actionReport?.repo ||
      snap.rows.find((r) => r.def.key === "RELEASE_REPOSITORY")?.display ||
      repos[0]?.name ||
      "",
  );
  const selected = repos.find((r) => r.name === repo);
  const report = snap.actionReport;
  const labels: Record<string, string> = {
    success: t("成功", "Success", "成功"),
    failure: t("失敗", "Failed", "失败"),
    cancelled: t("キャンセル", "Cancelled", "已取消"),
    timed_out: t("時間切れ", "Timed out", "超时"),
    in_progress: t("実行中", "In progress", "运行中"),
    queued: t("待機中", "Queued", "排队中"),
    waiting: t("承認待ち", "Waiting", "等待中"),
    skipped: t("スキップ", "Skipped", "已跳过"),
    action_required: t("操作が必要", "Action required", "需要操作"),
    completed: t("完了", "Completed", "已完成"),
    neutral: t("中立", "Neutral", "中性"),
    requested: t("要求済み", "Requested", "已请求"),
    pending: t("保留中", "Pending", "待处理"),
    stale: t("期限切れ", "Stale", "已过期"),
  };
  return (
    <div className="actions-report">
      <label htmlFor="actions-repo">
        {t("リポジトリ", "Repository", "仓库")}
      </label>
      {connection ? (
        <select
          id="actions-repo"
          value={repo}
          onChange={(e) => choose(e.target.value)}
          disabled={busy}
        >
          <option value="">
            {t("選択してください", "Select repository", "请选择仓库")}
          </option>
          {repos.map((r) => (
            <option key={r.id} value={r.name}>
              {r.name}
            </option>
          ))}
        </select>
      ) : (
        <input
          id="actions-repo"
          value={repo}
          onChange={(e) => choose(e.target.value)}
          placeholder="owner/repository"
          disabled={busy}
        />
      )}
      {connection && !repos.length && (
        <p>
          {t(
            "GitHub Appの権限設定で、対象リポジトリのActionsを「読み取り」に設定してください。GitHub側での権限追加後はリポジトリを再取得します。",
            "Allow Actions read access for this repository in GitHub App settings. Refresh repositories after updating the permissions on GitHub.",
            "请在 GitHub App 权限设置中为目标仓库启用 Actions 读取权限。在 GitHub 更新权限后，请刷新仓库。",
          )}
        </p>
      )}
      <button
        type="button"
        disabled={busy || !repo.trim() || Boolean(connection && !selected)}
        onClick={() =>
          action("actions-fetch", {
            repo: repo.trim(),
            repo_id: selected?.id || "",
            installation_id: connection?.installation || "",
          })
        }
      >
        <RefreshCw size={14} />
        {t("最新20件を取得", "Fetch latest 20 runs", "获取最近20次运行")}
      </button>
      {report && report.repo === repo.trim() && (
        <>
          <small>
            {t("取得日時", "Fetched", "获取时间")} ·{" "}
            {new Date(report.checked).toLocaleString(
              document.documentElement.lang,
            )}
          </small>
          {!report.runs.length && (
            <p>
              {t(
                "実行履歴はありません。",
                "No workflow runs.",
                "暂无运行记录。",
              )}
            </p>
          )}
          <div className="run-list">
            {report.runs.map((run) => {
              const state = run.conclusion || run.status;
              return (
                <article key={run.id}>
                  <div>
                    <strong>{run.name || "Workflow"}</strong>
                    <span className={"badge run-" + state}>
                      {labels[state] || state}
                    </span>
                    <button
                      type="button"
                      className="icon"
                      disabled={busy}
                      title={t(
                        "GitHubで実行を開く",
                        "Open run on GitHub",
                        "在 GitHub 查看运行",
                      )}
                      onClick={() =>
                        action("actions-open", {
                          repo: report.repo,
                          run_id: String(run.id),
                        })
                      }
                    >
                      <ExternalLink size={14} />
                    </button>
                  </div>
                  <small>
                    {run.branch} · <code>{run.sha.slice(0, 7)}</code> ·{" "}
                    {run.event} · #{run.id} / {run.attempt}
                  </small>
                  <small>
                    {new Date(run.updated).toLocaleString(
                      document.documentElement.lang,
                    )}
                  </small>
                </article>
              );
            })}
          </div>
          <small className="run-note">
            <Info size={13} />
            {t(
              "リポジトリ全体の履歴です。Secret登録後の実行かは、ブランチ・コミット・日時で確認してください。",
              "Repository-wide history. Check branch, commit and time to identify runs after your secret update.",
              "此处显示整个仓库的历史。请核对分支、提交和时间，以确认更新机密后的运行。",
            )}
          </small>
        </>
      )}
    </div>
  );
}
