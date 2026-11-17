import { useState } from "react";
import { Archive, FolderClosed, KeyRound, LockKeyhole, Settings, Rows3 } from "lucide-react";
import { Language } from "./controls";
import type { Lang } from "./guide";
import type { Scope, Snapshot, T } from "./types";

export function Preferences({
  snap, t, lang, density, textSize, changeLang, changeDensity, changeTextSize, selectProject, show, busy,
}: {
  snap: Snapshot;
  t: T;
  lang: Lang;
  density: string;
  textSize: string;
  changeTextSize: (value: string) => void;
  changeLang: (lang: Lang) => void;
  changeDensity: (value: string) => void;
  show: (modal: string) => void;
  busy: boolean;
  selectProject: (scope: Scope) => void;
}) {
  const [section, setSection] = useState<"general" | "appearance">("general");
  const general = t("一般", "General", "通用");
  const appearance = t("アピアランス", "Appearance", "外观");
  return (
    <div className="preferences">
      <nav aria-label={t("環境設定", "Settings", "偏好设置")}>
        <button aria-pressed={section === "general"} onClick={() => setSection("general")}>
          <Settings size={16} />{general}
        </button>
        <button aria-pressed={section === "appearance"} onClick={() => setSection("appearance")}>
          <Rows3 size={16} />{appearance}
        </button>
      </nav>
      <section>
        <h1>{section === "general" ? general : appearance}</h1>
        {section === "general" ? (
          <>
            <div className="preference-row">
              <span>{t("言語", "Language", "语言")}</span>
              <Language lang={lang} change={changeLang} />
            </div>
            {snap.locked ? (
              <p>{t("保管庫の設定を変更するにはメイン画面でロックを解除してください。", "Unlock the vault in the main window to change vault settings.", "请在主窗口解锁保险库以更改设置。")}</p>
            ) : (
              <div className="preference-actions">
                <label className="preference-row">
                  {t("プロジェクト", "Project", "项目")}
                  <select
                    value={snap.scope.project}
                    disabled={busy}
                    onChange={(event) => {
                      const id = Number(event.currentTarget.value);
                      const project = snap.projects.find((item) => item.id === id);
                      if (!project || !project.targets.length) return;
                      selectProject({
                        ...snap.scope,
                        project: project.id,
                        target: project.targets.includes(snap.scope.target)
                          ? snap.scope.target
                          : project.targets[0],
                      });
                    }}
                  >
                    {snap.projects.map((project) => (
                      <option key={project.id} value={project.id}>{project.name}</option>
                    ))}
                  </select>
                </label>
                <button disabled={busy} onClick={() => show("edit-project")}>
                  <FolderClosed size={16} />
                  {t("プロジェクト設定を開く", "Open project settings", "打开项目设置")}
                </button>
                <button disabled={busy} onClick={() => show("backup")}>
                  <Archive size={16} />{t("バックアップ", "Backup", "备份")}
                </button>
                {(snap.deviceAvailable || snap.deviceEnabled) && (
                  <button disabled={busy} onClick={() => show(snap.deviceEnabled ? "device-disable" : "device-enable")}>
                    <LockKeyhole size={16} />{t("Macの認証設定", "Mac authentication", "Mac 身份验证")}
                  </button>
                )}
                {(snap.passkeyAvailable || snap.passkeyEnabled) && (
                  <button disabled={busy} onClick={() => show(snap.passkeyEnabled ? "passkey-disable" : "passkey-enable")}>
                    <KeyRound size={16} />{t("パスキー設定", "Passkey settings", "通行密钥设置")}
                  </button>
                )}
              </div>
            )}
          </>
        ) : (
          <>
            <label className="preference-row">
              {t("文字サイズ", "Text size", "文字大小")}
              <select value={textSize} onChange={(e) => changeTextSize(e.target.value)}>
                <option value="small">{t("小", "Small", "小")}</option>
                <option value="standard">{t("標準", "Standard", "标准")}</option>
                <option value="large">{t("大", "Large", "大")}</option>
                <option value="extra-large">{t("特大", "Extra large", "特大")}</option>
              </select>
            </label>
          <label className="preference-row">
            {t("資格情報シートの行間隔", "Credential sheet row spacing", "凭据表行距")}
            <select value={density} onChange={(e) => changeDensity(e.target.value)}>
              <option value="standard">{t("標準", "Standard", "标准")}</option>
              <option value="compact">{t("コンパクト", "Compact", "紧凑")}</option>
            </select>
          </label>
          </>
        )}
      </section>
    </div>
  );
}
