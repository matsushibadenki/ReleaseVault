import { ExternalLink, FileDiff } from "lucide-react";
import type { Sheet, T, FormValues } from "./types";

export function WorkflowPreview({
  snap,
  t,
  busy,
  action,
}: {
  snap: Sheet;
  t: T;
  busy: boolean;
  action: (op: string, data: FormValues) => void;
}) {
  const preview = snap.workflowPreview;
  if (!preview)
    return (
      <p>
        {t(
          "差分を取得してください。",
          "Fetch a preview first.",
          "请先获取差异预览。",
        )}
      </p>
    );
  return (
    <div className="workflow-preview">
      <strong>{preview.repo}</strong>
      <small>
        {preview.base} · <code>{preview.baseSha.slice(0, 7)}</code>
      </small>
      <code className="workflow-path">{preview.path}</code>
      <input type="hidden" name="preview_id" value={preview.id} />
      {preview.changed ? (
        <pre
          tabIndex={0}
          aria-label={t("変更差分", "Proposed diff", "更改差异")}
        >
          {preview.diff.split("\n").map((line, index) => (
            <span
              key={index}
              className={
                line.startsWith("+")
                  ? "diff-added"
                  : line.startsWith("-")
                    ? "diff-removed"
                    : undefined
              }
            >
              {line}
              {"\n"}
            </span>
          ))}
        </pre>
      ) : (
        <p>
          {t(
            "既存のワークフローと同じ内容です。",
            "The existing workflow already matches.",
            "内容与现有工作流一致。",
          )}
        </p>
      )}
      {preview.pr ? (
        <button
          type="button"
          disabled={busy}
          onClick={() => action("workflow-open", {})}
        >
          <ExternalLink size={14} />
          {t("GitHubでPRを開く", "Open PR on GitHub", "在 GitHub 打开 PR")} #
          {preview.pr}
        </button>
      ) : (
        preview.changed && (
          <label className="review-consent">
            <input type="checkbox" required />
            {t(
              "対象リポジトリと差分を確認しました。",
              "I reviewed the repository and proposed changes.",
              "我已核对目标仓库及更改内容。",
            )}
          </label>
        )
      )}
      {snap.githubApp &&
        !snap.githubApp.repos.some(
          (repo) =>
            repo.name === preview.repo &&
            repo.grants.contents === "write" &&
            repo.grants.workflows === "write" &&
            repo.grants.pull_requests === "write",
        ) && (
          <small>
            {t(
              "PR作成にはContents・Workflows・Pull requestsの「読み書き」権限を設定してください。",
              "Grant Contents, Workflows and Pull requests write access to create a PR.",
              "创建 PR 需要 Contents、Workflows 和 Pull requests 的写入权限。",
            )}
          </small>
        )}
      {preview.changed && !preview.pr && (
        <small>
          <FileDiff size={13} />
          {t(
            "専用ブランチにこのファイルを保存し、Draft PRを作成します。",
            "Saves this file on a dedicated branch and creates a draft PR.",
            "将在专用分支保存此文件并创建草稿 PR。",
          )}
        </small>
      )}
    </div>
  );
}
