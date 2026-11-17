import { Preferences } from "./Preferences";
import { createRoot } from "react-dom/client";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { errorText } from "./messages";
import { Workspace } from "./Workspace";
import { Dialog } from "./Dialog";
import { Gate } from "./Gate";
import "./style.css";
import { useState, useEffect, useRef, useCallback } from "react";
import { X, LoaderCircle } from "lucide-react";
import { translate, type Lang } from "./guide";
import { status, type Snapshot, type FormValues } from "./types";
import { Mark, Btn, floating } from "./controls";
function App() {
  const [lang, setLang] = useState<Lang>(() => {
    const s = localStorage.getItem("rv.language") || navigator.language;
    return s.startsWith("ja") ? "ja" : s.startsWith("zh") ? "zh" : "en";
  });
  const [snap, setSnap] = useState<Snapshot>(),
    [note, notice] = useState(""),
    [busy, setBusy] = useState(false),
    [checkingRemote, setCheckingRemote] = useState(false),
    [modal, show] = useState(""),
    [revealed, reveal] = useState("");
  const active = useRef(false),
    generation = useRef(0),
    secretTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined),
    t = translate(lang);
  const changeLang = (l: Lang) => {
    setLang(l);
    localStorage.setItem("rv.language", l);
  };
  const [density, setDensity] = useState(
    () => localStorage.getItem("rv.density") || "standard",
  );
  const [textSize, setTextSize] = useState(
    () => localStorage.getItem("rv.textSize") || "standard",
  );
  useEffect(() => {
    document.documentElement.dataset.textSize = textSize;
  }, [textSize]);
  useEffect(() => {
    document.documentElement.dataset.density = density;
  }, [density]);
  useEffect(() => {
    const sync = (event: StorageEvent) => {
      if (event.key === "rv.language" && ["ja", "zh", "en"].includes(event.newValue || ""))
        setLang(event.newValue as Lang);
      if (event.key === "rv.textSize") setTextSize(event.newValue || "standard");
      if (event.key === "rv.density") setDensity(event.newValue || "standard");
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);
  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    const n = ++generation.current;
    const v = await invoke<Snapshot>("snapshot");
    if (n === generation.current) setSnap(v);
  }, []);
  const run = async (
    fn: () => Promise<unknown>,
    done?: (v: unknown) => void,
  ) => {
    if (active.current) return;
    active.current = true;
    setBusy(true);
    notice("");
    try {
      const v = await fn();
      await refresh();
      done?.(v);
    } catch (e) {
      if (String(e) !== "cancel") notice(errorText(String(e), t));
      await refresh().catch(() => {});
    } finally {
      active.current = false;
      setBusy(false);
    }
  };
  useEffect(() => {
    document.documentElement.lang = lang === "zh" ? "zh-CN" : lang;
  }, [lang]);
  useEffect(() => {
    if (!isTauri()) return;
    let live = true;
    const l = listen("workspace-changed", () => {
      if (live) refresh().catch(() => notice("Vault unavailable"));
    });
    refresh().catch(() => notice("Vault unavailable"));
    const poll = setInterval(() => refresh().catch(() => {}), 10000);
    return () => {
      live = false;
      l.then((f) => f());
      clearInterval(poll);
    };
  }, [refresh]);
  const identity =
    snap && !snap.locked
      ? JSON.stringify([
          snap.scope,
          snap.selected,
          snap.githubApp?.installation,
        ])
      : "locked";
  useEffect(() => {
    show("");
    reveal("");
    notice("");
    clearTimeout(secretTimer.current);
  }, [identity]);
  useEffect(() => {
    if (!snap || snap.locked) return;
    let last = 0;
    const touch = () => {
      if (Date.now() - last > 10000) {
        last = Date.now();
        invoke("touch").catch(() => refresh());
      }
    };
    window.addEventListener("pointerdown", touch);
    window.addEventListener("keydown", touch);
    const blur = () => reveal("");
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("pointerdown", touch);
      window.removeEventListener("keydown", touch);
      window.removeEventListener("blur", blur);
    };
  }, [snap?.locked, refresh]);
  useEffect(() => {
    if (note) {
      const id = setTimeout(() => notice(""), 9500);
      return () => clearTimeout(id);
    }
  }, [note]);
  const row =
    snap && !snap.locked
      ? snap.rows.find((x) => x.def.key === snap.selected) || snap.rows[0]
      : undefined;
  const action = (op: string, data: FormValues = {}) => {
    if (!snap || snap.locked || active.current) return;
    if (op === "release-check" || op === "apple-fetch") setCheckingRemote(true);
    run(
      () =>
        invoke("perform", {
          op,
          scope: snap.scope,
          input: { key: row?.def.key || "", ...data },
        }),
      (v) => {
        if (op === "apple-fetch" || op === "apple-apply") show("apple");
        else if (
          op === "workflow-preview" ||
          op === "workflow-create" ||
          op === "workflow-open"
        )
          show("workflow-review");
        else if (
          op === "release-add" ||
          op === "release-remove" ||
          op === "release-export" ||
          op === "release-check"
        )
          show("release");
        else if (op === "app-refresh" || op === "app-permission")
          show("app-permission");
        else if (op !== "app-settings" && !op.startsWith("actions-")) show("");
        if (op === "reveal") {
          reveal(String(v));
          clearTimeout(secretTimer.current);
          secretTimer.current = setTimeout(() => reveal(""), 15000);
        } else if (
          op !== "apple-fetch" &&
          !op.startsWith("workflow-") &&
          !op.startsWith("release-") &&
          !op.startsWith("actions-") &&
          op !== "source" &&
          op !== "acquire" &&
          op !== "app-settings"
        ) {
          notice(
            op === "github"
              ? t(
                  "GitHubが登録を受理しました。",
                  "GitHub accepted the update.",
                  "GitHub 已接受更新。",
                )
              : op === "check-conf" || op === "verify"
                ? t(
                    "照合に成功しました。",
                    "Verification passed.",
                    "验证通过。",
                  )
                : t("完了しました。", "Completed.", "已完成。"),
          );
        }
      },
    ).finally(() => {
      if (op === "release-check" || op === "apple-fetch")
        setCheckingRemote(false);
    });
  };
  const lock = () =>
    run(
      () => invoke("lock"),
      () => {
        reveal("");
        show("");
      },
    );
  return (
    <>
      <div className={busy ? "shell working" : "shell"} aria-busy={busy}>
        {!isTauri() ? (
          <div className="gate">
            <Mark />
            <h1>ReleaseVault</h1>
            <p>
              {t(
                "保管庫を使用するにはデスクトップアプリを起動してください。",
                "Open the desktop app to access your vault.",
                "请打开桌面应用以访问保险库。",
              )}
            </p>
          </div>
        ) : !snap ? (
          <div className="gate">
            <Mark />
            <h1>ReleaseVault</h1>
            <LoaderCircle className="spin" />
          </div>
        ) : new URLSearchParams(location.search).has("preferences") ? (
          <Preferences
            {...{ snap, t, lang, density, textSize, show, busy }}
            changeTextSize={(value) => {
              setTextSize(value);
              localStorage.setItem("rv.textSize", value);
            }}
            changeLang={changeLang}
            selectProject={(scope) => run(() => invoke("select", { scope, key: "" }))}
            changeDensity={(value) => {
              setDensity(value);
              localStorage.setItem("rv.density", value);
            }}
          />
        ) : snap.locked ? (
          <Gate
            key={String(snap.exists)}
            exists={snap.exists}
            deviceEnabled={snap.deviceEnabled}
            passkeyEnabled={snap.passkeyEnabled}
            t={t}
            lang={lang}
            change={changeLang}
            busy={busy}
            submit={(mode, password) =>
              run(() => invoke("authenticate", { mode, password }))
            }
          />
        ) : (
          <Workspace
            key={String(snap.scope.project)}
            {...{
              snap,
              row,
              lang,
              t,
              changeLang,
              busy,
              show,
              action,
              lock,
              revealed,
            }}
            select={(scope, key) => run(() => invoke("select", { scope, key }))}
            openPreferences={() => run(() => invoke("open_preferences"))}
            toggle={() =>
              run(() => invoke("float_panel", { floating: !snap.floating }))
            }
          />
        )}
        {modal && snap && !snap.locked && (
          <Dialog
            key={modal + identity}
            {...{ modal, row, snap, t, busy, show, action }}
            lockDownload={
              checkingRemote
                ? () => {
                    invoke("lock")
                      .then(refresh)
                      .catch((e) => notice(errorText(String(e), t)));
                  }
                : undefined
            }
            cancelDownload={
              checkingRemote
                ? () => {
                    invoke("cancel_remote_check").catch((e) =>
                      notice(errorText(String(e), t)),
                    );
                  }
                : undefined
            }
            message={note}
            close={() => {
              show("");
              notice("");
            }}
            submit={(data) =>
              action(
                modal === "choose-import"
                  ? String(data.encoding)
                  : modal === "workflow-review"
                    ? "workflow-create"
                    : modal,
                data,
              )
            }
          />
        )}
      </div>
      {note && (
        <aside className="toast" role="status">
          {note}
          <Btn
            icon={X}
            title={t("閉じる", "Close", "关闭")}
            click={() => notice("")}
          />
        </aside>
      )}
      {busy && <div className="progress" />}
    </>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
