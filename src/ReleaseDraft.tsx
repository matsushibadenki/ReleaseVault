import { FileJson, Plus, Trash2, Info, ShieldCheck } from "lucide-react";
import type { Sheet, T, FormValues } from "./types";
export function ReleaseDraft({
  snap,
  t,
  busy,
  action,
  show,
}: {
  snap: Sheet;
  t: T;
  busy: boolean;
  action: (op: string, data: FormValues) => void;
  show: (modal: string) => void;
}) {
  const draft = snap.releaseDraft;
  const supported = [
    "mac-direct",
    "windows-direct",
    "windows-exe",
    "linux-appimage",
  ].includes(snap.scope.target);
  const results: Record<string, string> = {
    verified: t(
      "公開ファイルの署名が一致",
      "Hosted signature verified",
      "托管文件签名一致",
    ),
    mismatch: t("署名が一致しません", "Signature mismatch", "签名不匹配"),
    version: t(
      "署名のバージョンが異なります",
      "Signed version mismatch",
      "签名版本不一致",
    ),
    "download-missing": t(
      "公開ファイルが見つかりません",
      "Hosted file not found",
      "找不到托管文件",
    ),
    "download-denied": t(
      "公開先へのアクセスが拒否されました",
      "Access denied by host",
      "托管方拒绝访问",
    ),
    download: t("取得できませんでした", "Download failed", "下载失败"),
    "download-timeout": t(
      "取得が時間切れになりました",
      "Download timed out",
      "下载超时",
    ),
    "download-size": t(
      "2 GiBの取得上限を超えています",
      "Exceeds the 2 GiB download limit",
      "超过 2 GiB 下载限制",
    ),
    "download-url": t(
      "HTTPSの転送先を確認してください",
      "Check the HTTPS destination",
      "请检查 HTTPS 目标",
    ),
    "download-redirects": t(
      "転送回数が上限を超えました",
      "Too many redirects",
      "重定向次数过多",
    ),
    format: t(
      "署名の形式を確認してください",
      "Check the signature format",
      "请检查签名格式",
    ),
    cancel: t("取得を中止しました", "Download cancelled", "已取消下载"),
  };
  return (
    <div className="actions-report">
      <div className="release-context">
        <strong>{snap.scope.env}</strong>
        {draft && <code>v{draft.version}</code>}
      </div>
      {draft?.platforms.map((entry) => (
        <article className="release-platform" key={entry.platform}>
          <div>
            <strong>{entry.platform}</strong>
            <button
              type="button"
              className="icon"
              disabled={busy}
              title={t(
                "削除して再検証",
                "Remove to reverify",
                "移除并重新验证",
              )}
              onClick={() =>
                action("release-remove", { platform: entry.platform })
              }
            >
              <Trash2 size={14} />
            </button>
          </div>
          <small>{entry.url}</small>
          <small>
            {t("署名検証", "Signature verified", "签名验证")} ·{" "}
            {new Date(entry.verified).toLocaleString(
              document.documentElement.lang,
            )}
          </small>
          {entry.keyCurrent === false ? (
            <small className="remote-failure">
              {t(
                "公開鍵が変更されています。削除して再検証してください。",
                "Public key changed. Remove and verify this platform again.",
                "公钥已更改，请移除并重新验证此平台。",
              )}
            </small>
          ) : entry.remote ? (
            <small
              className={
                entry.remote.status === "verified"
                  ? "remote-success"
                  : "remote-failure"
              }
            >
              {results[entry.remote.status] ||
                t(
                  "照合できませんでした",
                  "Verification failed",
                  "验证失败",
                )}{" "}
              ·{" "}
              {new Date(entry.remote.checked).toLocaleString(
                document.documentElement.lang,
              )}
              {entry.remote.bytes != null &&
                ` · ${(entry.remote.bytes / 1024 / 1024).toFixed(2)} MiB`}
            </small>
          ) : (
            <small>
              {t(
                "公開ファイル未照合",
                "Hosted file not checked",
                "尚未核对托管文件",
              )}
            </small>
          )}
          <button
            type="button"
            disabled={busy || entry.keyCurrent === false}
            onClick={() =>
              action("release-check", { platform: entry.platform })
            }
          >
            <ShieldCheck size={14} />
            {t("公開ファイルを照合", "Verify hosted file", "验证托管文件")}
          </button>
        </article>
      ))}
      {!draft && (
        <p>
          {t(
            "配布先ごとに署名と成果物を検証して追加します。",
            "Verify and add the signature and artifact for each distribution target.",
            "请按分发目标验证签名和产物并添加。",
          )}
        </p>
      )}
      <button
        type="button"
        disabled={busy || !supported}
        onClick={() => show("release-add")}
      >
        <Plus size={14} />
        {t("現在の配布先を追加", "Add current target", "添加当前目标")}
      </button>
      <button type="button" disabled={busy} onClick={() => show("workflow")}>
        <FileJson size={14} />
        {t(
          "ビルド用Actionsを書き出す",
          "Export build workflow",
          "导出构建工作流",
        )}
      </button>
      {!supported && (
        <small>
          {t(
            "macOS・Windowsの直接配布、またはLinux AppImageを選択してください。",
            "Select direct distribution for macOS or Windows, or Linux AppImage.",
            "请选择 macOS 或 Windows 直接分发，或 Linux AppImage。",
          )}
        </small>
      )}
      <button
        type="button"
        className="primary"
        disabled={busy || !draft?.platforms.length}
        onClick={() => action("release-export", {})}
      >
        <FileJson size={14} />
        {t("まとめてJSONを書き出す", "Export combined JSON", "导出合并的 JSON")}
      </button>
      <small className="run-note">
        <Info size={13} />
        {t(
          "公開ファイルの照合は記録したURLから取得し、登録済みの署名・公開鍵・バージョンで検証します。別のOSを追加するときは配布先を切り替えます。",
          "Hosted verification downloads the recorded URL and checks its signature, public key and version. Switch distribution targets to add another OS.",
          "托管验证从记录的 URL 下载文件，并核对签名、公钥和版本。添加其他系统时，请切换分发目标。",
        )}
      </small>
    </div>
  );
}
