import { useState } from "react";
import { LockKeyhole, HardDrive, KeyRound } from "lucide-react";
import { type Lang } from "./guide";
import { type T } from "./types";
import { Mark, Language } from "./controls";
export function Gate({
  exists,
  deviceEnabled,
  passkeyEnabled,
  t,
  lang,
  change,
  busy,
  submit,
}: {
  exists: boolean;
  deviceEnabled: boolean;
  passkeyEnabled: boolean;
  t: T;
  lang: Lang;
  change: (l: Lang) => void;
  busy: boolean;
  submit: (op: string, p: string) => void;
}) {
  const [restore, setRestore] = useState(false);
  const mode = exists ? "unlock" : restore ? "restore" : "create";
  const label = exists
    ? t("ロック解除", "Unlock vault", "解锁保险库")
    : restore
      ? t("バックアップから復元", "Restore backup", "恢复备份")
      : t("保管庫を作成", "Create vault", "创建保险库");
  const [mismatch, setMismatch] = useState(false);
  return (
    <div className="lockscreen">
      <div className="gate">
        <Mark />
        <h1>ReleaseVault</h1>
        <p>
          {exists && (passkeyEnabled || deviceEnabled)
            ? t("保管庫のロックを解除", "Unlock your vault", "解锁保险库")
            : exists
              ? t(
                  "マスターパスワードでロックを解除",
                  "Unlock with your master password",
                  "使用主密码解锁",
                )
              : restore
                ? t(
                    "バックアップ作成時のパスワードを入力",
                    "Enter the password used for the backup",
                    "输入创建备份时使用的密码",
                  )
                : t(
                    "署名情報を保管するマスターパスワードを設定",
                    "Choose a master password for your signing credentials",
                    "为签名凭据设置主密码",
                  )}
        </p>
        <form
          key={mode}
          onSubmit={(e) => {
            e.preventDefault();
            const f = e.currentTarget;
            const p = f.elements.namedItem("password") as HTMLInputElement;
            if (
              mode === "create" &&
              p.value !==
                (f.elements.namedItem("confirmation") as HTMLInputElement).value
            ) {
              setMismatch(true);
              return;
            }
            setMismatch(false);
            submit(mode, p.value);
            f.reset();
          }}
        >
          <label className="input-icon">
            <LockKeyhole size={16} />
            <input
              name="password"
              type="password"
              minLength={mode === "create" ? 12 : 1}
              maxLength={4096}
              autoComplete={exists ? "current-password" : "new-password"}
              placeholder={
                mode === "create"
                  ? t(
                      "パスワード（12文字以上）",
                      "Password · at least 12 characters",
                      "密码（至少 12 个字符）",
                    )
                  : t("マスターパスワード", "Master password", "主密码")
              }
              aria-label={t("マスターパスワード", "Master password", "主密码")}
              required
              autoFocus
            />
          </label>
          {mode === "create" && (
            <input
              name="confirmation"
              type="password"
              autoComplete="new-password"
              placeholder={t(
                "パスワードを再入力",
                "Confirm password",
                "确认密码",
              )}
              required
              aria-label={t("確認", "Confirm password", "确认密码")}
            />
          )}
          {mismatch && (
            <small role="alert">
              {t(
                "パスワードが一致しません。",
                "Passwords do not match.",
                "密码不一致。",
              )}
            </small>
          )}
          <button className="primary" disabled={busy}>
            {label}
          </button>
        </form>
        {exists && passkeyEnabled && (
          <button disabled={busy} onClick={() => submit("passkey", "")}>
            <KeyRound size={14} />
            {t("パスキーで解除", "Unlock with a passkey", "使用通行密钥解锁")}
          </button>
        )}
        {exists && deviceEnabled && (
          <button disabled={busy} onClick={() => submit("device", "")}>
            <LockKeyhole size={14} />
            {t(
              "Macの認証で解除",
              "Unlock with Mac authentication",
              "使用 Mac 身份验证解锁",
            )}
          </button>
        )}
        {!exists && (
          <button
            className="link"
            onClick={() => {
              setRestore(!restore);
              setMismatch(false);
            }}
          >
            {restore
              ? t("新しい保管庫を作成", "Create a new vault", "创建新保险库")
              : t("バックアップから復元", "Restore from backup", "从备份恢复")}
          </button>
        )}
      </div>
      <div className="gate-foot">
        <HardDrive size={14} /> Local vault{" "}
        <Language lang={lang} change={change} />
      </div>
    </div>
  );
}
