import React from "react";
import { Inspector } from "./Inspector";
import { useState, useEffect, useRef } from "react";
import {
  KeyRound,
  LockKeyhole,
  FolderClosed,
  Github,
  Plus,
  Search,
  ListFilter,
  Table2,
  History,
  PanelRightClose,
  Upload,
  Download,
  SlidersHorizontal,
  Layers3,
  HardDrive,
  ArrowUpDown,
  Menu,
  Check,
  Monitor,
  Archive,
  FileBadge,
  Columns3,
} from "lucide-react";
import { credentialSet, sheetGroups } from "./credentialSets";
import { credentialName, targets } from "./guide";
import { status, type Row, type Scope, type T, type WorkProps } from "./types";
import { Mark, Btn, Language, floating } from "./controls";
function badge(r: Row, t: T) {
  return {
    missing: t("未登録", "Missing", "未配置"),
    configured: t("設定済", "Configured", "已配置"),
    expired: t("期限切れ", "Expired", "已过期"),
    soon: t("期限間近", "Due soon", "即将到期"),
  }[status(r)];
}
export function Workspace(p: WorkProps) {
  const {
    snap: s,
    row,
    t,
    lang,
    changeLang,
    show,
    action,
    lock,
    select,
    toggle,
  } = p;
  const project = s.projects.find((x) => x.id === s.scope.project)!;
  const [query, search] = useState(""),
    [filter, setFilter] = useState("all"),
    [sort, order] = useState(false),
    [view, setView] = useState("sheet"),
    [nav, setNav] = useState(false),
    [narrowDetail, detail] = useState(false),
    [keys, showKeys] = useState(true);
  const [marked, mark] = useState<string[]>([]);
  const anchor = useRef("");
  useEffect(() => {
    mark([]);
    anchor.current = "";
  }, [s.scope.project, s.scope.target, s.scope.env]);
  const rows = s.rows.filter((r) => {
    const set = credentialSet(r.def);
    return (
      (filter === "all" || status(r) === filter) &&
      [r.def.key, credentialName(r.def, t), r.def.name, set && t(...set.name)]
        .join(" ")
        .toLowerCase()
        .includes(query.trim().toLowerCase())
    );
  });
  if (sort)
    rows.sort((a, b) =>
      credentialName(a.def, t).localeCompare(credentialName(b.def, t), lang),
    );
  const groups = sheetGroups(rows, t),
    ready = s.rows.filter((r) => r.present).length;
  const clickRow = (e: React.MouseEvent, r: Row) => {
    const k = r.def.key;
    if (e.metaKey || e.ctrlKey) {
      mark((prev) =>
        prev.includes(k) ? prev.filter((v) => v !== k) : [...prev, k],
      );
      anchor.current = k;
      return;
    }
    if (e.shiftKey && anchor.current) {
      const ordered = groups.flatMap((g) => g.members.map((v) => v.def.key));
      const a = ordered.indexOf(anchor.current),
        b = ordered.indexOf(k);
      if (a >= 0) {
        mark(ordered.slice(Math.min(a, b), Math.max(a, b) + 1));
        return;
      }
    }
    mark([]);
    anchor.current = k;
    choose(s.scope, k);
    detail(true);
  };
  const copyMetadata = () => {
    const selected = s.rows.filter((r) => marked.includes(r.def.key));
    const str = selected
      .map((r) => [credentialName(r.def, t), r.def.key, badge(r, t)].join("\t"))
      .join("\n");
    return str;
  };
  const choose = (scope: Scope, k = "") => {
    select(scope, k);
    setNav(false);
    if (!k) {
      search("");
      setFilter("all");
      detail(false);
    }
  };
  if (floating) return row ? <Inspector {...p} row={row} /> : null;
  return (
    <div
      className={
        "work" +
        (s.floating ? " detached" : "") +
        (nav ? " nav-open" : "") +
        (narrowDetail ? " detail-open" : "")
      }
    >
      <aside className="sidebar">
        <div className="brand">
          <Mark />
          ReleaseVault
        </div>
        <div className="subhead">
          {t("プロジェクト", "Projects", "项目")}
          <Btn
            icon={Plus}
            title={t("プロジェクトを追加", "Add project", "添加项目")}
            click={() => show("project")}
          />
        </div>
        {s.projects.map((x) => (
          <button
            key={x.id}
            className={"nav " + (project.id === x.id ? "active" : "")}
            onClick={() =>
              choose({ ...s.scope, project: x.id, target: x.targets[0] })
            }
          >
            <FolderClosed />
            {x.name}
          </button>
        ))}
        <div className="subhead">
          {t("配布先", "Distribution targets", "分发目标")}
        </div>
        {project.targets.map((x) => (
          <button
            key={x}
            className={"nav target " + (s.scope.target === x ? "chosen" : "")}
            onClick={() => choose({ ...s.scope, target: x })}
          >
            <Monitor />
            <span>
              {targets[x].split(" · ")[0]} ·{" "}
              <span className="keep">{targets[x].split(" · ")[1]}</span>
            </span>
          </button>
        ))}
        <div className="sidefoot">
          <button
            className="nav"
            onClick={() => show(s.githubApp ? "app-permission" : "app-connect")}
          >
            <Github />
            GitHub App
          </button>
          {(s.passkeyAvailable || s.passkeyEnabled) && (
            <button
              className="nav"
              onClick={() =>
                show(s.passkeyEnabled ? "passkey-disable" : "passkey-enable")
              }
            >
              <KeyRound />
              {t("パスキー設定", "Passkey settings", "通行密钥设置")}
            </button>
          )}
          {(s.deviceAvailable || s.deviceEnabled) && (
            <button
              className="nav"
              onClick={() =>
                show(s.deviceEnabled ? "device-disable" : "device-enable")
              }
            >
              <LockKeyhole />
              {t("Macの認証設定", "Mac authentication", "Mac 身份验证")}
            </button>
          )}
          <button className="nav" onClick={() => show("edit-project")}>
            <SlidersHorizontal />
            {t("プロジェクト設定", "Project settings", "项目设置")}
          </button>
          <button className="nav" onClick={() => show("backup")}>
            <Archive />
            {t("バックアップ", "Backup", "备份")}
          </button>
          <button className="nav" onClick={lock}>
            <LockKeyhole />
            {t("今すぐロック", "Lock vault", "锁定保险库")}
          </button>
          <Language lang={lang} change={changeLang} />
        </div>
      </aside>
      {nav && (
        <button
          className="scrim"
          aria-label="Close navigation"
          onClick={() => setNav(false)}
        />
      )}
      <main>
        <header>
          <div className="crumb">
            <Btn
              icon={Menu}
              title={t("ナビゲーション", "Navigation", "导航")}
              click={() => setNav(!nav)}
            />
            <FolderClosed size={13} />
            {project.name}
            <span>/</span>
            {targets[s.scope.target]}
          </div>
          <div className="headline">
            <h1>
              {view === "sheet"
                ? t("資格情報シート", "Credential sheet", "凭据表")
                : t("操作履歴", "Activity log", "操作记录")}
            </h1>
            <select
              value={s.scope.env}
              aria-label={t("環境", "Environment", "环境")}
              onChange={(e) => choose({ ...s.scope, env: e.target.value })}
            >
              {["Development", "Staging", "Production"].map((x) => (
                <option key={x}>{x}</option>
              ))}
            </select>
          </div>
          <div className="stats">
            <span>
              {t("項目", "Items", "项目")} <b>{s.rows.length}</b>
            </span>
            <span>
              {t("設定済", "Configured", "已配置")}{" "}
              <b className="teal">{ready}</b>
            </span>
            <span>
              {t("未登録", "Missing", "未配置")} <b>{s.rows.length - ready}</b>
            </span>
            <span className="spacer" />
            <Btn
              icon={view === "sheet" ? History : Table2}
              title={
                view === "sheet"
                  ? t("操作履歴", "Activity log", "操作记录")
                  : t("資格情報シート", "Credential sheet", "凭据表")
              }
              click={() => setView(view === "sheet" ? "audit" : "sheet")}
            />
          </div>
        </header>
        {view === "sheet" ? (
          <>
            <div className="toolbar">
              <label className="input-icon">
                <Search size={16} />
                <input
                  placeholder={t(
                    "項目名・キー名を検索",
                    "Search credentials…",
                    "搜索名称或密钥…",
                  )}
                  value={query}
                  onChange={(e) => search(e.target.value)}
                />
              </label>
              <label className="filter">
                <ListFilter size={14} />
                <select
                  aria-label="Status"
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                >
                  <option value="all">
                    {t("全状態", "All statuses", "全部状态")}
                  </option>
                  {["missing", "configured", "soon", "expired"].map((v, i) => (
                    <option key={v} value={v}>
                      {
                        [
                          t("未登録", "Missing", "未配置"),
                          t("設定済", "Configured", "已配置"),
                          t("期限間近", "Due soon", "即将到期"),
                          t("期限切れ", "Expired", "已过期"),
                        ][i]
                      }
                    </option>
                  ))}
                </select>
              </label>
              <Btn
                icon={Plus}
                title={t("項目を追加", "Add credential", "添加凭据")}
                click={() => show("custom")}
              />
            </div>
            <div
              className="sheet-scroll"
              onCopy={(e) => {
                if (marked.length) {
                  e.preventDefault();
                  e.clipboardData.setData("text/plain", copyMetadata());
                }
              }}
            >
              <table className={!keys ? "hide-key" : ""}>
                <thead>
                  <tr>
                    <th>
                      <button className="th" onClick={() => order(!sort)}>
                        {t("項目", "Credential", "凭据")}
                        <ArrowUpDown size={12} />
                      </button>
                    </th>
                    <th className="keycol">
                      {t("キー名", "Environment key", "密钥名称")}
                    </th>
                    <th>{t("状態", "Status", "状态")}</th>
                  </tr>
                </thead>
                {groups.map((g) => (
                  <tbody
                    key={g.id}
                    className={g.name ? "credential-set" : undefined}
                  >
                    <tr className="group">
                      <th colSpan={3} scope="rowgroup">
                        <div>
                          {g.name && (
                            <Layers3
                              size={13}
                              aria-label={t("セット", "Set", "组合")}
                            />
                          )}
                          <span>{g.provider}</span>
                          {g.name && (
                            <>
                              <span aria-hidden="true">/</span>
                              <strong>{g.name}</strong>
                            </>
                          )}
                          <small>{g.members.length}</small>
                        </div>
                      </th>
                    </tr>
                    {g.members.map((r) => {
                      const I =
                        r.def.kind === "secret"
                          ? KeyRound
                          : r.def.kind === "id"
                            ? FileBadge
                            : SlidersHorizontal;
                      return (
                        <tr
                          key={r.def.key}
                          tabIndex={0}
                          aria-selected={r.def.key === row?.def.key}
                          className={
                            "credential " +
                            (marked.includes(r.def.key) ||
                            (!marked.length && r.def.key === row?.def.key)
                              ? "selected"
                              : "")
                          }
                          onClick={(e) => clickRow(e, r)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              choose(s.scope, r.def.key);
                              detail(true);
                            }
                          }}
                        >
                          <td>
                            <div>
                              <I size={15} />
                              {credentialName(r.def, t)}
                            </div>
                          </td>
                          <td className="keycol">
                            <code>{r.def.key}</code>
                          </td>
                          <td>
                            <span className={"badge " + status(r)}>
                              {r.present ? <Check size={11} /> : <i />}
                              {badge(r, t)}
                            </span>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                ))}
              </table>
              {!rows.length && (
                <div className="empty">
                  <Search />
                  {t(
                    "一致する項目がありません",
                    "No matching credentials",
                    "没有匹配的凭据",
                  )}
                </div>
              )}
            </div>
            <footer>
              {marked.length > 0 && <span>{marked.length} · ⌘/Ctrl+C</span>}
              <HardDrive size={13} />
              {t("ローカル保管庫", "Local vault", "本地保险库")}
              <span className="spacer" />
              <Btn
                icon={Columns3}
                title={t("キー列の表示切替", "Toggle key column", "切换密钥列")}
                click={() => showKeys(!keys)}
              />
              <Btn
                icon={Upload}
                title={t("CSVを読み込む", "Import CSV", "导入 CSV")}
                click={() => action("csv-import")}
              />
              <Btn
                icon={Download}
                title={t(
                  "CSVを書き出す（値なし）",
                  "Export CSV without values",
                  "导出 CSV（不含值）",
                )}
                click={() => action("csv-export")}
              />
              {s.floating && (
                <Btn
                  icon={PanelRightClose}
                  title={t("パネルを戻す", "Dock inspector", "停靠面板")}
                  click={toggle}
                />
              )}
            </footer>
          </>
        ) : (
          <section className="audit">
            {!s.audit.length ? (
              <div className="empty">
                <History />
                {t("履歴はまだありません", "No activity yet", "暂无操作记录")}
              </div>
            ) : (
              s.audit.map((a, i) => (
                <article key={i}>
                  <span>{new Date(a.at).toLocaleString(lang)}</span>
                  <b>{eventName(a.action, t)}</b>
                  <code>{a.key}</code>
                </article>
              ))
            )}
          </section>
        )}
      </main>
      {!s.floating && row && (
        <Inspector {...p} row={row} back={() => detail(false)} />
      )}
    </div>
  );
}
function eventName(a: string, t: T) {
  return (
    (
      {
        save: t("保存", "Saved", "已保存"),
        "app-connect": t(
          "GitHub Appに接続",
          "Connected GitHub App",
          "已连接 GitHub App",
        ),
        "app-disconnect": t(
          "GitHub Appの接続を解除",
          "Disconnected GitHub App",
          "已断开 GitHub App",
        ),
        "app-inspect": t(
          "GitHub登録日時を照会",
          "Checked GitHub timestamp",
          "已查询 GitHub 更新时间",
        ),
        "app-refresh": t(
          "リポジトリ一覧を更新",
          "Refreshed repositories",
          "已刷新仓库",
        ),
        "app-permission": t(
          "リポジトリ権限を変更",
          "Changed repository access",
          "已更改仓库权限",
        ),
        github: t("GitHubに送信", "Sent to GitHub", "已发送至 GitHub"),
        generate: t("鍵生成", "Generated keys", "已生成密钥"),
        verify: t("署名検証", "Verified signature", "已验证签名"),
        "config-match": t("設定照合", "Matched configuration", "配置匹配"),
        link: t("共有元に接続", "Linked to source", "已关联来源"),
        shared: t("共有先を追加", "Added linked project", "已添加引用项目"),
        unlink: t("参照を解除", "Removed reference", "已取消引用"),
        unshared: t("共有先を解除", "Removed linked project", "已移除引用项目"),
        "source-changed": t(
          "共有元の値が更新されました",
          "Source value updated",
          "来源值已更新",
        ),
        "key-match": t(
          "証明書と秘密鍵が一致",
          "Matched private key",
          "证书与私钥匹配",
        ),
        certificate: t("証明書解析", "Parsed certificate", "已解析证书"),
        reveal: t("一時表示", "Revealed", "已显示"),
        clear: t("値を削除", "Cleared", "已清除"),
        add: t("項目追加", "Added credential", "已添加凭据"),
        "csv-import": t("CSV読込", "Imported CSV", "已导入 CSV"),
      } as Record<string, string>
    )[a] || a
  );
}
