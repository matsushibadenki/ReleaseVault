import { useState, useEffect, useRef } from "react";
import { ExternalLink, X } from "lucide-react";
import { credentialName, targets } from "./guide";
import { type Row, type Sheet, type T, type FormValues } from "./types";
import { AppleResources } from "./AppleResources";
import { WorkflowPreview } from "./WorkflowPreview";
import { ReleaseDraft } from "./ReleaseDraft";
import { ActionsReport } from "./ActionsReport";
import { GitHubAccess } from "./GitHubAccess";
import references from "../resources/sources.json";
import { Btn } from "./controls";
const examples: Record<string, { example: string | string[] }> =
  references.items;

function valueExample(row: Row, t: T) {
  const sample = row.def.guide !== "custom" && examples[row.def.key]?.example;
  if (Array.isArray(sample)) return t(sample[0], sample[1], sample[2]);
  if (sample) return sample;
  return row.def.kind === "secret"
    ? "Example-Only!42"
    : row.def.kind === "id"
      ? "example-id"
      : "example-value";
}

export function Dialog({
  modal: m,
  row,
  snap,
  t,
  busy,
  message,
  cancelDownload,
  lockDownload,
  close,
  submit,
  show,
  action,
}: {
  modal: string;
  row?: Row;
  snap: Sheet;
  t: T;
  busy: boolean;
  message: string;
  cancelDownload?: () => void;
  lockDownload?: () => void;
  close: () => void;
  show: (modal: string) => void;
  action: (op: string, data: FormValues) => void;
  submit: (d: FormValues) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  const workflowCanCreate =
    !snap.githubApp ||
    Boolean(
      snap.githubApp.repos.find(
        (repo) =>
          repo.name === snap.workflowPreview?.repo &&
          repo.grants.contents === "write" &&
          repo.grants.workflows === "write" &&
          repo.grants.pull_requests === "write",
      ),
    );
  const secret = row?.def.kind === "secret",
    affected = ["save", "choose-import", "certificate", "key-match"].includes(m)
      ? (row?.dependents ?? [])
      : [],
    needsPass =
      (m === "workflow-review" &&
        workflowCanCreate &&
        Boolean(snap.workflowPreview?.changed) &&
        !snap.workflowPreview?.pr) ||
      affected.length > 0 ||
      [
        "github",
        "app-connect",
        "app-permission",
        "app-disconnect",
        "generate",
        "backup",
        "reveal",
        "clear",
        "share",
        "unlink",
        "device-enable",
        "device-disable",
        "passkey-enable",
        "passkey-disable",
      ].includes(m);
  const appPassword =
    row?.def.key === "APPLE_PASSWORD" && row.def.guide === "apple-account";
  const editingAppId =
    m === "save" &&
    row?.def.key === "APPLE_APP_ID" &&
    row.def.guide === "apple-id";
  const identifier = (field: string) =>
    snap.rows
      .find((r) => r.def.key === field && !r.truncated)
      ?.display?.trim() || "";
  const bundle = editingAppId ? identifier("APPLE_BUNDLE_ID") : "";
  const team = editingAppId ? identifier("APPLE_TEAM_ID") : "";
  const suffix = "." + bundle;
  const currentId = row?.display || "";
  const prefixEditor = Boolean(
    bundle && (currentId ? currentId.endsWith(suffix) : team),
  );
  const [appPrefix, setAppPrefix] = useState(() =>
    currentId.endsWith(suffix) ? currentId.slice(0, -suffix.length) : team,
  );
  const existing = snap.projects.find((p) => p.id === snap.scope.project)!;
  const workflowOptions = [
    {
      platform: "darwin-aarch64",
      name: "macOS · Apple Silicon",
      target: "mac-direct",
    },
    { platform: "darwin-x86_64", name: "macOS · Intel", target: "mac-direct" },
    {
      platform: "windows-x86_64",
      name: "Windows · x64",
      target: "windows-direct",
    },
    {
      platform: "linux-x86_64",
      name: "Linux AppImage · x64",
      target: "linux-appimage",
    },
  ].filter(
    (option) =>
      existing.targets.includes(option.target) ||
      (option.target === "windows-direct" &&
        existing.targets.includes("windows-exe")),
  );
  const [workflowTargets, setWorkflowTargets] = useState(() =>
    workflowOptions.map((option) => option.platform),
  );
  const workflowRepos =
    snap.githubApp?.repos.filter((repo) =>
      ["read", "write"].includes(repo.grants.contents ?? "none"),
    ) ?? [];
  const [workflowRepo, setWorkflowRepo] = useState(
    snap.rows.find((row) => row.def.key === "RELEASE_REPOSITORY")?.display ||
      workflowRepos[0]?.name ||
      "",
  );
  const workflowRepository = workflowRepos.find(
    (repo) => repo.name === workflowRepo,
  );
  const isProject = m === "project" || m === "edit-project";
  const [picked, pick] = useState<string[]>(
      m === "edit-project" ? existing.targets : ["mac-direct", "mac-store"],
    ),
    [repo, setRepo] = useState(
      snap.rows.find((x) => x.def.key === "RELEASE_REPOSITORY")?.display || "",
    ),
    [env, setEnv] = useState(snap.githubApp ? "" : snap.scope.env);
  const app = snap.githubApp;
  const inspect = m === "app-inspect";
  const eligible =
    app?.repos.filter((r) => {
      const level =
        r.grants[env ? "environments" : secret ? "secrets" : "variables"];
      return inspect ? level !== "none" : level === "write";
    }) ?? [];
  const selectedRepo = eligible.find((r) => r.name === repo);
  const titles: Record<string, string> = {
    "app-inspect": t(
      "GitHubの登録日時を照会",
      "Check GitHub timestamp",
      "查询 GitHub 更新时间",
    ),
    "app-connect": t(
      "GitHub Appに接続",
      "Connect GitHub App",
      "连接 GitHub App",
    ),
    "app-permission": t("リポジトリの権限", "Repository access", "仓库权限"),
    "app-disconnect": t(
      "プロジェクトの接続を解除",
      "Disconnect project",
      "断开项目连接",
    ),
    share: t("共有元を選択", "Select source project", "选择来源项目"),
    unlink: t(
      "このプロジェクトの参照を解除",
      "Unlink from this project",
      "取消本项目的引用",
    ),
    actions: t(
      "Actionsの実行結果",
      "Actions workflow runs",
      "Actions 运行记录",
    ),
    "key-match": t(
      "証明書を選んで照合",
      "Choose certificate and verify",
      "选择证书并验证",
    ),
    certificate: t(
      "関連証明書を選択",
      "Choose related certificate",
      "选择关联证书",
    ),
    "passkey-enable": t("パスキーを登録", "Register a passkey", "注册通行密钥"),
    "passkey-disable": t(
      "保管庫のパスキー解除を無効化",
      "Disable passkey unlock for this vault",
      "停用此保险库的通行密钥解锁",
    ),
    "device-enable": t(
      "Macの認証を有効化",
      "Enable Mac authentication",
      "启用 Mac 身份验证",
    ),
    "device-disable": t(
      "Macの認証を無効化",
      "Disable Mac authentication",
      "停用 Mac 身份验证",
    ),
    "edit-project": t("プロジェクト設定", "Project settings", "项目设置"),
    apple: t("Appleの有効期限", "Apple expiry dates", "Apple 到期日期"),
    "workflow-review": t(
      "ワークフローの差分",
      "Workflow changes",
      "工作流差异",
    ),
    workflow: t(
      "ビルド用Actionsを書き出す",
      "Export build workflow",
      "导出构建工作流",
    ),
    release: t("更新JSONまとめ", "Combined update JSON", "更新 JSON 汇总"),
    "release-add": t(
      "署名を検証して追加",
      "Verify and add platform",
      "验证并添加平台",
    ),
    manifest: t("更新JSONを生成", "Create update manifest", "生成更新清单"),
    save: t("値を編集", "Edit value", "编辑值"),
    project: t("プロジェクトを追加", "New project", "新建项目"),
    custom: t("項目を追加", "New credential", "添加凭据"),
    backup: t("暗号化バックアップ", "Encrypted backup", "加密备份"),
    clear: t("保存した値を削除", "Clear stored value", "清除已保存的值"),
    github: t("GitHubに登録", "Send to GitHub", "发送到 GitHub"),
    generate: t("更新鍵ペアを生成", "Generate updater keys", "生成更新密钥对"),
    reveal: t("15秒間表示", "Reveal for 15 seconds", "显示 15 秒"),
    "choose-import": t("ファイルを読み込む", "Import file", "导入文件"),
  };
  return (
    <dialog
      ref={dialog}
      onCancel={(e) => {
        e.preventDefault();
        if (!busy) close();
      }}
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (m === "actions" || m === "release" || m === "apple") return;
          const f = e.currentTarget;
          const values = Object.fromEntries(new FormData(f)) as Record<
            string,
            string
          >;
          if (isProject && !picked.length) return;
          if (m === "workflow" && !workflowTargets.length) return;
          submit({
            ...values,
            affected: affected.map(String),
            ...(isProject ? { targets: picked } : {}),
            ...(m === "workflow" ? { targets: workflowTargets } : {}),
          });
          if (needsPass) {
            const pass = f.elements.namedItem("password") as HTMLInputElement;
            if (pass) pass.value = "";
          }
        }}
      >
        <div className="dialog-head">
          <h2>{titles[m]}</h2>
          <Btn
            icon={X}
            title={t("閉じる", "Close", "关闭")}
            click={close}
            disabled={busy}
          />
        </div>
        {![
          "project",
          "edit-project",
          "custom",
          "backup",
          "actions",
          "release",
          "workflow",
          "workflow-review",
          "apple",
          "device-enable",
          "device-disable",
          "passkey-enable",
          "passkey-disable",
        ].includes(m) &&
          !m.startsWith("app-") && (
            <code className="keylabel">{row?.def.key}</code>
          )}
        {["project", "edit-project", "custom"].includes(m) && (
          <label>
            {t("名前", "Name", "名称")}
            <input
              name="name"
              defaultValue={m === "edit-project" ? existing.name : ""}
              maxLength={100}
              required
              autoFocus
            />
          </label>
        )}
        {isProject && (
          <fieldset>
            <legend>{t("配布先", "Distribution targets", "分发目标")}</legend>
            <div className="checks">
              {Object.entries(targets).map(([k, n]) => (
                <label key={k}>
                  <input
                    type="checkbox"
                    checked={picked.includes(k)}
                    onChange={() =>
                      pick(
                        picked.includes(k)
                          ? picked.filter((x) => x !== k)
                          : [...picked, k],
                      )
                    }
                  />
                  {n}
                </label>
              ))}
            </div>
          </fieldset>
        )}
        {m === "custom" && (
          <>
            <label>
              {t("キー名", "Key name", "密钥名称")}
              <input
                name="key"
                pattern="[A-Z_][A-Z0-9_]*"
                placeholder="SIGNING_KEY"
                required
              />
            </label>
            <label>
              {t("種類", "Type", "类型")}
              <select name="kind">
                <option value="secret">
                  {t("機密情報", "Secret", "机密信息")}
                </option>
                <option value="id">
                  {t("識別子", "Identifier", "标识符")}
                </option>
                <option value="config">
                  {t("設定値", "Configuration", "配置值")}
                </option>
              </select>
            </label>
          </>
        )}
        {m === "share" && (
          <label>
            {t("共有元", "Source project", "来源项目")} · {snap.scope.env}
            <select name="owner" required>
              {row?.sources.map((id) => (
                <option key={id} value={id}>
                  {snap.projects.find((p) => p.id === id)?.name}
                </option>
              ))}
            </select>
          </label>
        )}
        {m === "unlink" && (
          <p>
            {t(
              "共有元の値は引き続き共有元で管理できます。",
              "The source project keeps its credential.",
              "来源项目将保留此凭据。",
            )}
          </p>
        )}
        {affected.length > 0 && (
          <p>
            {t(
              "変更を反映するプロジェクト",
              "Projects affected by this change",
              "本次更改影响的项目",
            )}{" "}
            ·{" "}
            {affected
              .map((id) => snap.projects.find((p) => p.id === id)?.name)
              .join("、")}{" "}
            · {snap.scope.env}
          </p>
        )}
        {m === "key-match" && (
          <label>
            {t(
              "鍵ファイルのパスワード（必要な場合）",
              "Key file password (if required)",
              "密钥文件密码（如需要）",
            )}
            <input name="key_password" type="password" autoComplete="off" />
          </label>
        )}
        {m === "passkey-enable" && (
          <p>
            {t(
              "パスキーを作成し、続けて同じパスキーで本人確認します。復元にはマスターパスワードが必要です。",
              "Create a passkey, then verify it to finish setup. Keep your master password for recovery.",
              "创建通行密钥后，再使用该密钥验证一次。恢复保险库仍需主密码。",
            )}
          </p>
        )}
        {m === "device-enable" && (
          <p>
            {t(
              "このMacの認証で保管庫を解除できます。復元にはマスターパスワードが必要です。",
              "Use this Mac’s authentication to unlock. Keep your master password for recovery.",
              "使用此 Mac 的身份验证解锁。恢复时仍需主密码。",
            )}
          </p>
        )}
        {m === "apple" && (
          <AppleResources snap={snap} t={t} busy={busy} action={action} />
        )}
        {m === "workflow-review" && (
          <WorkflowPreview snap={snap} t={t} busy={busy} action={action} />
        )}
        {m === "workflow" && (
          <>
            <fieldset>
              <legend>
                {t("ビルドするOS・CPU", "Build platforms", "构建平台")}
              </legend>
              <div className="checks">
                {workflowOptions.map((option) => (
                  <label key={option.platform}>
                    <input
                      type="checkbox"
                      checked={workflowTargets.includes(option.platform)}
                      onChange={(e) =>
                        setWorkflowTargets((prev) =>
                          e.target.checked
                            ? [...prev, option.platform]
                            : prev.filter((value) => value !== option.platform),
                        )
                      }
                    />
                    {option.name}
                  </label>
                ))}
              </div>
            </fieldset>
            {!workflowOptions.length && (
              <p>
                {t(
                  "プロジェクトに直接配布先を追加してください。",
                  "Add a direct distribution target to this project.",
                  "请为此项目添加直接分发目标。",
                )}
              </p>
            )}
            <small>
              {t(
                "選択した環境のSecrets・Variablesを使い、手動実行でビルドします。各OSの成果物と署名を1つのダウンロードにまとめます。",
                "Builds manually using secrets and variables from the selected environment. Collects each platform’s artifacts and signatures in one download.",
                "使用所选环境的机密和变量手动构建，将各平台产物及签名汇总为一个下载。",
              )}
            </small>
            <small>
              {t(
                "保存したYAMLをリポジトリの.github/workflowsに配置してください。",
                "Place the exported YAML in the repository’s .github/workflows directory.",
                "请将导出的 YAML 放入仓库的 .github/workflows 目录。",
              )}
            </small>
            <label>
              {t("PRの対象リポジトリ", "Repository for PR", "PR 目标仓库")}
              {app ? (
                <select
                  value={workflowRepo}
                  onChange={(e) => setWorkflowRepo(e.target.value)}
                >
                  <option value="">
                    {t("選択してください", "Select repository", "请选择仓库")}
                  </option>
                  {workflowRepos.map((repo) => (
                    <option key={repo.id} value={repo.name}>
                      {repo.name}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  value={workflowRepo}
                  onChange={(e) => setWorkflowRepo(e.target.value)}
                  placeholder="owner/repository"
                />
              )}
            </label>
            <button
              type="button"
              disabled={
                busy ||
                !workflowTargets.length ||
                !workflowRepo.trim() ||
                Boolean(app && !workflowRepository)
              }
              onClick={() =>
                action("workflow-preview", {
                  repo: workflowRepo.trim(),
                  repo_id: workflowRepository?.id || "",
                  installation_id: app?.installation || "",
                  targets: workflowTargets,
                })
              }
            >
              {t(
                "差分を取得して確認",
                "Preview repository changes",
                "获取并预览更改",
              )}
            </button>
            {app && !workflowRepos.length && (
              <small>
                {t(
                  "GitHub AppのContents読み取り権限を設定し、リポジトリを再取得してください。",
                  "Grant Contents read access in GitHub App settings and refresh repositories.",
                  "请设置 GitHub App 的 Contents 读取权限并刷新仓库。",
                )}
              </small>
            )}
          </>
        )}
        {m === "release" && (
          <ReleaseDraft
            snap={snap}
            t={t}
            busy={busy}
            action={action}
            show={show}
          />
        )}
        {m === "actions" && (
          <ActionsReport snap={snap} t={t} busy={busy} action={action} />
        )}
        {m === "save" && (
          <>
            <div className="example-field">
              <label htmlFor="credential-value">
                {prefixEditor
                  ? t("App ID Prefix", "App ID Prefix", "App ID 前缀")
                  : row &&
                      (row.def.guide === "apple-account" ||
                        (row.def.key === "APPLE_APP_ID" &&
                          row.def.guide === "apple-id"))
                    ? credentialName(row.def, t)
                    : row?.present
                      ? t("置き換える値", "Replacement value", "替换值")
                      : t("値", "Value", "值")}
              </label>
              {prefixEditor ? (
                <>
                  <input
                    id="credential-value"
                    value={appPrefix}
                    onChange={(e) => setAppPrefix(e.target.value)}
                    aria-describedby="app-id-preview value-example"
                    required
                    autoFocus
                    pattern="[A-Za-z0-9]+"
                    spellCheck={false}
                    autoCapitalize="none"
                  />
                  <input
                    type="hidden"
                    name="value"
                    value={appPrefix ? appPrefix + suffix : ""}
                  />
                  <small id="app-id-preview">
                    {t("保存するApp ID", "App ID to save", "将保存的 App ID")} ·{" "}
                    <output>{appPrefix ? appPrefix + suffix : "—"}</output>
                  </small>
                  {team && appPrefix !== team && (
                    <button
                      type="button"
                      className="link"
                      onClick={() => setAppPrefix(team)}
                    >
                      {t("Team IDを使う", "Use Team ID", "使用 Team ID")}
                    </button>
                  )}
                </>
              ) : secret ? (
                <input
                  type="password"
                  id="credential-value"
                  aria-describedby="value-example"
                  name="value"
                  autoComplete="new-password"
                  inputMode={row?.def.key === "APPLE_ID" ? "email" : undefined}
                  required={!(appPassword && row?.present)}
                  placeholder={
                    appPassword && row?.present
                      ? t(
                          "変更する場合のみ入力",
                          "Enter to replace",
                          "仅在更改时输入",
                        )
                      : undefined
                  }
                  autoFocus
                />
              ) : (
                <textarea
                  id="credential-value"
                  aria-describedby="value-example"
                  name="value"
                  defaultValue={row?.truncated ? "" : row?.display || ""}
                  required
                  rows={3}
                  autoFocus
                />
              )}
              {row && (
                <small id="value-example">
                  {t("例：", "Example: ", "示例：")}
                  {prefixEditor ? "AB12C3D4E5" : valueExample(row, t)}
                </small>
              )}
            </div>
            {appPassword && (
              <div className="example-field">
                <label htmlFor="issued-label">
                  {t(
                    "発行時に付けた名前（任意）",
                    "Name given when issued (optional)",
                    "创建时填写的名称（可选）",
                  )}
                </label>
                <input
                  id="issued-label"
                  aria-describedby="label-example"
                  name="app_password_label"
                  maxLength={200}
                  defaultValue={row?.appPasswordLabel ?? ""}
                />
                <small id="label-example">
                  {t(
                    "例：macOS公証用",
                    "Example: macOS notarization",
                    "示例：macOS 公证",
                  )}
                </small>
              </div>
            )}
            <div className="example-field">
              <label htmlFor="credential-expiry">
                {t("有効期限（任意）", "Expiry (optional)", "有效期（可选）")}
              </label>
              <input
                id="credential-expiry"
                aria-describedby="expiry-example"
                type="date"
                name="expires"
                defaultValue={row?.expires || ""}
              />
              <small id="expiry-example">
                {t(
                  "例：2030年12月31日",
                  "Example: December 31, 2030",
                  "示例：2030年12月31日",
                )}
              </small>
            </div>
          </>
        )}
        {m === "choose-import" && (
          <label>
            {t("保存形式", "Encoding", "保存格式")}
            <select name="encoding">
              <option value="import-file">
                {t(
                  "Base64（.p12、JKS、バイナリ）",
                  "Base64 (.p12, JKS, binary)",
                  "Base64（.p12、JKS、二进制）",
                )}
              </option>
              <option value="import-text">
                {t(
                  "元のテキスト（.p8、PEM、Tauri鍵）",
                  "Original text (.p8, PEM, Tauri keys)",
                  "原始文本（.p8、PEM、Tauri 密钥）",
                )}
              </option>
            </select>
          </label>
        )}
        {app && (
          <input
            type="hidden"
            name="installation_id"
            value={app.installation}
          />
        )}
        {m === "app-connect" && (
          <>
            <label>
              App ID
              <input
                name="app_id"
                inputMode="numeric"
                pattern="[1-9][0-9]*"
                required
                autoFocus
              />
            </label>
            <label>
              Installation ID
              <input
                name="installation_id"
                inputMode="numeric"
                pattern="[1-9][0-9]*"
                required
              />
            </label>
            <small>
              {t(
                "続けてGitHub Appの秘密鍵（.pem）を選択します。",
                "Next, choose the GitHub App private key (.pem).",
                "接下来请选择 GitHub App 私钥（.pem）。",
              )}
            </small>
          </>
        )}
        {m === "app-permission" && app && (
          <GitHubAccess
            connection={app}
            {...{ busy, t, action }}
            disconnect={() => show("app-disconnect")}
          />
        )}
        {m === "app-disconnect" && (
          <p>
            {t(
              "この保管庫から接続鍵と権限設定を削除し、自動監視を停止します。",
              "Remove the connection key and repository grants from this vault and stop monitoring this project.",
              "从本保险库中删除连接密钥和仓库权限，并停止监控此项目。",
            )}
          </p>
        )}
        {(m === "github" || inspect) && (
          <>
            <label>
              {t("リポジトリ", "Repository", "仓库")}
              <input
                type="hidden"
                name="provider"
                value={app ? "app" : "cli"}
              />
              {app ? (
                <>
                  <input
                    type="hidden"
                    name="repo_id"
                    value={selectedRepo?.id ?? ""}
                  />
                  <select
                    name="repo"
                    value={selectedRepo?.name ?? ""}
                    required
                    onChange={(e) => setRepo(e.target.value)}
                  >
                    <option value="">
                      {eligible.length
                        ? t(
                            "リポジトリを選択",
                            "Select repository",
                            "请选择仓库",
                          )
                        : inspect
                          ? t(
                              "読み取りを許可したリポジトリがありません",
                              "No repository has read access",
                              "没有允许读取的仓库",
                            )
                          : t(
                              "書き込みを許可したリポジトリがありません",
                              "No repository has write access",
                              "没有允许写入的仓库",
                            )}
                    </option>
                    {eligible.map((r) => (
                      <option key={r.id} value={r.name}>
                        {r.name}
                      </option>
                    ))}
                  </select>
                </>
              ) : (
                <input
                  name="repo"
                  placeholder="owner/repo"
                  value={repo}
                  onChange={(e) => setRepo(e.target.value)}
                  required
                  autoFocus
                />
              )}
            </label>
            <label>
              {t(
                "Environment（空欄でリポジトリ全体）",
                "Environment (blank for repository scope)",
                "环境（留空表示整个仓库）",
              )}
              <input
                name="environment"
                value={env}
                onChange={(e) => setEnv(e.target.value)}
              />
            </label>
            <div className="destination">
              <ExternalLink size={15} />
              <code>
                github.com/{(app ? selectedRepo?.name : repo) || "owner/repo"}
                <br />
                {env || "Repository"} / {secret ? "Secrets" : "Variables"} /{" "}
                {row?.def.key}
              </code>
            </div>
            <small>
              {app
                ? "GitHub App · " + app.account
                : t(
                    "同名の値を作成・更新します。GitHub CLIのログインが必要です。",
                    "Creates or replaces this value. Sign in with GitHub CLI first.",
                    "将创建或替换此值。请先登录 GitHub CLI。",
                  )}
            </small>
          </>
        )}
        {(m === "manifest" || m === "release-add") && (
          <>
            <p>
              {t(
                m === "release-add"
                  ? "続けて署名ファイル（.sig）と、その成果物を選びます。"
                  : "署名ファイル、成果物、保存先の順に選びます。選択したOSの単一プラットフォーム用です。",
                m === "release-add"
                  ? "Next, choose the .sig file and its artifact."
                  : "Choose a signature, artifact, then destination. The manifest contains this platform only.",
                m === "release-add"
                  ? "接下来请选择 .sig 文件及对应产物。"
                  : "依次选择签名、产物和保存位置。此清单仅包含当前平台。",
              )}
            </p>
            <label>
              Version
              <input
                name="version"
                placeholder="1.0.0"
                defaultValue={
                  m === "release-add" ? snap.releaseDraft?.version : ""
                }
                readOnly={m === "release-add" && Boolean(snap.releaseDraft)}
                required
              />
            </label>
            <label>
              Platform
              <select name="platform">
                {["aarch64", "x86_64", "i686"].map((a) => {
                  const v =
                    (snap.scope.target === "mac-direct"
                      ? "darwin"
                      : snap.scope.target.startsWith("windows")
                        ? "windows"
                        : "linux") +
                    "-" +
                    a;
                  return <option key={a}>{v}</option>;
                })}
              </select>
            </label>
            <label>
              HTTPS URL
              <input type="url" name="url" pattern="https://.+" required />
            </label>
          </>
        )}
        {m === "backup" && (
          <p>
            {t(
              "復元には現在のマスターパスワードが必要です。バックアップは別の安全な場所に保管してください。",
              "Restore with your current master password. Keep the backup in a separate, safe location.",
              "恢复时需要当前主密码。请将备份保存在独立的安全位置。",
            )}
          </p>
        )}
        {m === "clear" && row?.def.key.startsWith("TAURI_") && (
          <p>
            {t(
              "配布済みアプリの更新に使う鍵は、移行とバックアップを確認してから削除してください。",
              "Back up keys used by shipped versions and plan the transition before clearing them.",
              "清除已发布版本使用的密钥之前，请备份并确认迁移方案。",
            )}
          </p>
        )}
        {m === "generate" && (
          <p>
            {t(
              "秘密鍵・公開鍵・専用パスワードを生成します。すでに値がある場合は上書きしません。",
              "Creates a private key, public key, and password. Existing keys are never replaced.",
              "生成私钥、公钥和专用密码，不会覆盖已有密钥。",
            )}
          </p>
        )}
        {needsPass && (
          <label>
            {t("マスターパスワード", "Master password", "主密码")}
            <input
              name="password"
              type="password"
              autoComplete="current-password"
              required
              autoFocus={m !== "github"}
            />
          </label>
        )}
        {message && <small role="alert">{message}</small>}
        {lockDownload && (
          <button type="button" onClick={lockDownload}>
            {t("ロックして中止", "Lock and cancel", "锁定并取消")}
          </button>
        )}
        {cancelDownload && (
          <button type="button" onClick={cancelDownload}>
            {t("取得を中止", "Cancel download", "取消下载")}
          </button>
        )}
        <div className="dialog-foot">
          <button type="button" disabled={busy} onClick={close}>
            {m === "actions" ||
            m === "release" ||
            m === "workflow-review" ||
            m === "apple"
              ? t("閉じる", "Close", "关闭")
              : t("キャンセル", "Cancel", "取消")}
          </button>
          {m !== "actions" &&
            m !== "release" &&
            m !== "apple" &&
            (m !== "workflow-review" ||
              (snap.workflowPreview?.changed && !snap.workflowPreview.pr)) && (
              <button
                className={"primary" + (m === "clear" ? " danger" : "")}
                disabled={
                  busy ||
                  (isProject && !picked.length) ||
                  (m === "workflow" && !workflowTargets.length) ||
                  (m === "workflow-review" && !workflowCanCreate) ||
                  ((m === "github" || inspect) && !!app && !selectedRepo) ||
                  (m === "app-permission" && !app?.repos.length)
                }
              >
                {m === "workflow-review"
                  ? t("Draft PRを作成", "Create draft PR", "创建草稿 PR")
                  : m === "app-connect"
                    ? t(
                        "秘密鍵を選んで接続",
                        "Choose key and connect",
                        "选择私钥并连接",
                      )
                    : m === "app-permission"
                      ? t("権限を保存", "Save access", "保存权限")
                      : m === "app-disconnect"
                        ? t("接続を解除", "Disconnect", "断开连接")
                        : m === "github"
                          ? t("確認して送信", "Confirm and send", "确认发送")
                          : m === "save"
                            ? t("保存", "Save", "保存")
                            : titles[m]}
              </button>
            )}
        </div>
      </form>
    </dialog>
  );
}
