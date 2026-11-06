import { useRef, useState } from "react";
import { RefreshCw, CalendarCheck, Info } from "lucide-react";
import { credentialName } from "./guide";
import type { Sheet, T, FormValues } from "./types";

const certificateKeys = [
  "APPLE_CERTIFICATE",
  "APPLE_STORE_CERTIFICATE",
  "APPLE_INSTALLER_CERTIFICATE",
];
export function AppleResources({
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
  const report = snap.appleReport;
  const ready = ["APPLE_API_ISSUER", "APPLE_API_KEY", "APPLE_API_KEY_P8"].every(
    (key) => snap.rows.some((row) => row.def.key === key && row.present),
  );
  const [kind, setKind] = useState("certificates");
  const [resourceId, setResourceId] = useState("");
  const [targetKey, setTargetKey] = useState("");
  const password = useRef<HTMLInputElement>(null);
  const resources =
    report?.resources.filter((resource) => resource.kind === kind) ?? [];
  const resource =
    resources.find((resource) => resource.id === resourceId) || resources[0];
  const targets = snap.rows.filter(
    (row) =>
      row.present &&
      !row.owner &&
      row.def.guide !== "custom" &&
      (kind === "certificates"
        ? certificateKeys.includes(row.def.key)
        : row.def.key === "APPLE_PROVISIONING_PROFILE"),
  );
  const target = targets.find((row) => row.def.key === targetKey) || targets[0];
  const shared = Boolean(target?.dependents.length);
  return (
    <div className="apple-resources">
      <button
        type="button"
        disabled={busy || !ready}
        onClick={() => action("apple-fetch", {})}
      >
        <RefreshCw size={14} />
        {t(
          "Appleから期限を取得",
          "Fetch expiry dates from Apple",
          "从 Apple 获取到期日期",
        )}
      </button>
      {!ready && (
        <small>
          {t(
            "同じプロジェクト・環境にIssuer ID、Key ID、APIキー（.p8）を保存してください。",
            "Save the Issuer ID, Key ID and API key (.p8) in this project and environment.",
            "请在本项目和环境中保存 Issuer ID、Key ID 和 API 密钥（.p8）。",
          )}
        </small>
      )}
      {report && (
        <>
          <small>
            {t("取得日時", "Fetched", "获取时间")} ·{" "}
            {new Date(report.checked).toLocaleString(
              document.documentElement.lang,
            )}
          </small>
          {!report.credentialsCurrent && (
            <small className="remote-failure">
              {t(
                "APIキーが変更されています。期限を再取得してください。",
                "The API credentials changed. Fetch the expiry dates again.",
                "API 凭据已更改，请重新获取到期日期。",
              )}
            </small>
          )}
          <label>
            {t("種類", "Type", "类型")}
            <select
              aria-label={t("種類", "Type", "类型")}
              value={kind}
              disabled={busy}
              onChange={(e) => {
                setKind(e.target.value);
                setResourceId("");
                setTargetKey("");
              }}
            >
              <option value="certificates">
                {t("証明書", "Certificates", "证书")}
              </option>
              <option value="profiles">
                {t(
                  "プロビジョニングプロファイル",
                  "Provisioning profiles",
                  "描述文件",
                )}
              </option>
            </select>
          </label>
          {!resources.length ? (
            <p>
              {t(
                "取得した項目はありません。",
                "No resources returned.",
                "暂无获取的项目。",
              )}
            </p>
          ) : (
            <>
              <label>
                {t(
                  "Appleに登録された項目",
                  "Resource registered with Apple",
                  "Apple 登记的项目",
                )}
                <select
                  aria-label={t(
                    "Appleに登録された項目",
                    "Resource registered with Apple",
                    "Apple 登记的项目",
                  )}
                  value={resource?.id || ""}
                  disabled={busy}
                  onChange={(e) => setResourceId(e.target.value)}
                >
                  {resources.map((resource) => (
                    <option key={resource.id} value={resource.id}>
                      {resource.name} · {resource.expires}
                    </option>
                  ))}
                </select>
              </label>
              {resource && (
                <div className="apple-resource-meta">
                  <code>{resource.category}</code>
                  <span>
                    {t("有効期限", "Expires", "到期")} · {resource.expires}
                  </span>
                  {kind === "profiles" && (
                    <span>
                      {resource.state === "ACTIVE"
                        ? t("有効", "Active", "有效")
                        : t(
                            "無効・要確認",
                            "Invalid / review required",
                            "无效／需要检查",
                          )}
                    </span>
                  )}
                </div>
              )}
              <label>
                {t(
                  "期限を反映する保存済み項目",
                  "Saved credential to update",
                  "更新到期日期的已保存凭据",
                )}
                <select
                  aria-label={t(
                    "期限を反映する保存済み項目",
                    "Saved credential to update",
                    "更新到期日期的已保存凭据",
                  )}
                  value={target?.def.key || ""}
                  disabled={busy}
                  onChange={(e) => setTargetKey(e.target.value)}
                >
                  {!targets.length && (
                    <option value="">
                      {t(
                        "対応するファイルを先に保存してください。",
                        "Save the corresponding file first.",
                        "请先保存对应文件。",
                      )}
                    </option>
                  )}
                  {targets.map((target) => (
                    <option key={target.def.key} value={target.def.key}>
                      {credentialName(target.def, t)}
                    </option>
                  ))}
                </select>
              </label>
              {shared && (
                <>
                  <small>
                    {t(
                      "共有先にも反映されます。",
                      "Linked projects will also see the updated expiry.",
                      "引用此凭据的项目也会显示更新后的到期日期。",
                    )}
                  </small>
                  <label>
                    {t("マスターパスワード", "Master password", "主密码")}
                    <input
                      ref={password}
                      type="password"
                      required
                      autoComplete="current-password"
                    />
                  </label>
                </>
              )}
              <button
                type="button"
                disabled={
                  busy || !target || !resource || !report.credentialsCurrent
                }
                onClick={() => {
                  if (
                    !target ||
                    !resource ||
                    (shared && !password.current?.reportValidity())
                  )
                    return;
                  action("apple-apply", {
                    key: target.def.key,
                    report_id: report.id,
                    resource_id: resource.id,
                    resource_kind: kind,
                    affected: target.dependents.map(String),
                    password: password.current?.value || "",
                  });
                  if (password.current) password.current.value = "";
                }}
              >
                <CalendarCheck size={14} />
                {t(
                  "照合して期限を反映",
                  "Match and update expiry",
                  "匹配并更新到期日期",
                )}
              </button>
            </>
          )}
        </>
      )}
      <small className="run-note">
        <Info size={13} />
        {t(
          "チーム全体の一覧です。保存ファイルと一致した項目の期限を反映します。.p12は書き出し時のパスワードも保存してください。",
          "Lists the whole team. Expiry is applied only when the resource matches the saved file. Also save the export password for .p12 files.",
          "显示整个团队的列表，仅在与保存文件匹配时更新到期日期。请同时保存 .p12 的导出密码。",
        )}
      </small>
    </div>
  );
}
