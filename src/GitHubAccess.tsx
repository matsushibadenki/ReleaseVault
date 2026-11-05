import { useState } from "react";
import { ExternalLink, RefreshCw, Unplug } from "lucide-react";
import { Btn } from "./controls";
import type { AppConnection, FormValues, T } from "./types";

export function GitHubAccess({
  connection,
  t,
  busy,
  action,
  disconnect,
}: {
  connection: AppConnection;
  t: T;
  busy: boolean;
  action: (op: string, data: FormValues) => void;
  disconnect: () => void;
}) {
  const [id, select] = useState(connection.repos[0]?.id ?? "");
  const repo = connection.repos.find((r) => r.id === id);
  const options = ["none", "read", "write"] as const;
  const labels = {
    none: t("許可しない", "No access", "不允许"),
    read: t("読み取り", "Read", "只读"),
    write: t("読み書き", "Read and write", "读写"),
  };
  const args = { installation_id: connection.installation };
  return (
    <>
      <div className="app-account">
        <strong>{connection.account}</strong>
        <code>#{connection.installation}</code>
        <span className="spacer" />
        <Btn
          icon={RefreshCw}
          title={t("リポジトリを再取得", "Refresh repositories", "刷新仓库")}
          disabled={busy}
          click={() => action("app-refresh", args)}
        />
        <Btn
          icon={ExternalLink}
          title={t(
            "GitHubで対象を管理",
            "Manage installation on GitHub",
            "在 GitHub 管理安装",
          )}
          disabled={busy}
          click={() => action("app-settings", args)}
        />
        <Btn
          icon={Unplug}
          title={t(
            "このプロジェクトの接続を解除",
            "Disconnect this project",
            "断开本项目连接",
          )}
          disabled={busy}
          click={disconnect}
        />
      </div>
      {connection.repos.length === 0 ? (
        <p>
          {t(
            "GitHubでリポジトリを追加して再取得してください。",
            "Add repositories on GitHub, then refresh.",
            "请在 GitHub 添加仓库后刷新。",
          )}
        </p>
      ) : (
        <label>
          {t("リポジトリ", "Repository", "仓库")}
          <select
            required
            name="repo_id"
            value={repo?.id ?? ""}
            onChange={(e) => select(e.target.value)}
          >
            {!repo && (
              <option value="">
                {t("選択してください", "Select a repository", "请选择仓库")}
              </option>
            )}
            {connection.repos.map((r) => (
              <option key={r.id} value={r.id}>
                {r.name}
              </option>
            ))}
          </select>
        </label>
      )}
      {repo &&
        (["secrets", "variables", "environments"] as const).map((p) => (
          <label key={repo.id + p + repo.grants[p]}>
            {p === "environments"
              ? t(
                  "Environment の Secrets・Variables",
                  "Environment secrets and variables",
                  "环境机密和变量",
                )
              : p === "secrets"
                ? "Repository Secrets"
                : "Repository Variables"}
            <select name={p} defaultValue={repo.grants[p]}>
              {options
                .slice(0, options.indexOf(connection.limits[p]) + 1)
                .map((level) => (
                  <option value={level} key={level}>
                    {labels[level]}
                  </option>
                ))}
            </select>
          </label>
        ))}
    </>
  );
}
