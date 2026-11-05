import {
  KeyRound,
  SquareArrowOutUpRight,
  PanelRightClose,
  Upload,
  Download,
  FileKey2,
  SlidersHorizontal,
  ShieldCheck,
  Eye,
  Search,
  Trash2,
  Pencil,
  RefreshCw,
  ExternalLink,
  X,
  Check,
  FileBadge,
  Info,
  Link2,
  Unlink,
  Package,
} from "lucide-react";
import { credentialSet } from "./credentialSets";
import references from "../resources/sources.json";
import { credentialName, guide, type Lang } from "./guide";
import { type Row, type WorkProps } from "./types";
import { Btn, floating } from "./controls";
const helpItems: Record<string, { links: string; purpose: string[] }> =
  references.items;
const helpLinks: Record<
  string,
  { docs: string; get: string; local?: boolean; instruction?: string[] }
> = references.links;
const languageIndex: Record<Lang, number> = { ja: 0, en: 1, zh: 2 };
export function Inspector(p: WorkProps & { row: Row; back?: () => void }) {
  const { row: r, t, lang, show, action, toggle, revealed } = p,
    d = r.def;
  const set = credentialSet(d);
  const peers = set
    ? p.snap.rows.filter(
        (other) => other.def.key !== d.key && credentialSet(other.def) === set,
      )
    : [];
  const projectName = (id: number) =>
    p.snap.projects.find((x) => x.id === id)?.name ?? String(id);
  const linked = Boolean(r.owner);
  const users = r.dependents ?? [];
  const item = d.guide === "custom" ? undefined : helpItems[d.key];
  const links = item && helpLinks[item.links];
  const steps = links?.instruction
    ? [links.instruction[languageIndex[lang]]]
    : guide(d.guide, lang);
  return (
    <aside className="inspector">
      <div className="ins-head">
        <FileKey2 size={25} />
        <div>
          <h2>{credentialName(d, t)}</h2>
          <small>
            {d.group} ·{" "}
            {d.kind === "secret"
              ? t("機密情報", "Secret", "机密信息")
              : d.kind === "id"
                ? t("識別子", "Identifier", "标识符")
                : t("設定値", "Configuration", "配置值")}
          </small>
        </div>
        <Btn
          icon={floating ? PanelRightClose : SquareArrowOutUpRight}
          title={
            floating
              ? t("ドッキング", "Dock panel", "停靠面板")
              : t("パネルを分離", "Float inspector", "浮动面板")
          }
          click={toggle}
        />
        {p.back && (
          <span className="mobile-close">
            <Btn icon={X} title={t("閉じる", "Close", "关闭")} click={p.back} />
          </span>
        )}
      </div>
      <section>
        <h3>
          {t("値", "Value", "值")}
          <span
            title={t(
              "秘密値は再認証後に15秒間表示します。",
              "Secrets are shown for 15 seconds after authentication.",
              "重新验证后，机密值仅显示 15 秒。",
            )}
          >
            <Info size={13} />
          </span>
        </h3>
        <div
          className="value"
          onCopy={(e) => {
            if (d.kind === "secret") e.preventDefault();
          }}
        >
          <code>
            {revealed ||
              r.display ||
              (r.present
                ? "••••••••••••"
                : t("未登録", "Not configured", "未配置"))}
          </code>
          {r.present && d.kind === "secret" && (
            <Btn
              icon={Eye}
              title={t("一時表示", "Reveal temporarily", "临时显示")}
              click={() => show("reveal")}
            />
          )}
        </div>
        {r.appIdPrefix && (
          <small>
            {r.appIdPrefix === "team"
              ? t(
                  "Team IDとBundle IDから自動入力",
                  "Filled from Team ID and Bundle ID",
                  "由 Team ID 和 Bundle ID 自动填写",
                )
              : t(
                  "指定したPrefixとBundle IDから自動入力",
                  "Filled from your prefix and Bundle ID",
                  "由指定的前缀和 Bundle ID 自动填写",
                )}
          </small>
        )}
        {d.key === "APPLE_PASSWORD" && r.appPasswordLabel && (
          <small className="password-label">
            {t("発行時の名前", "Name given when issued", "创建时的名称")} ·{" "}
            {r.appPasswordLabel}
          </small>
        )}
        {r.owner && (
          <small>
            {t("共有元", "Source project", "来源项目")} · {projectName(r.owner)}
          </small>
        )}
        {users.length > 0 && (
          <small>
            {t("参照中のプロジェクト", "Linked projects", "引用项目")} ·{" "}
            {users.map(projectName).join("、")}
          </small>
        )}
        <div className="btnrow">
          <button disabled={linked} onClick={() => show("save")}>
            <Pencil size={13} />
            {t("編集", "Edit value", "编辑")}
          </button>
          <Btn
            icon={Upload}
            title={t("ファイルを読み込む", "Import file", "导入文件")}
            disabled={linked}
            click={() => show("choose-import")}
          />
          {r.present && !linked && r.appIdPrefix !== "team" && (
            <Btn
              icon={Trash2}
              title={t("値を削除", "Clear value", "删除值")}
              disabled={users.length > 0}
              click={() => show("clear")}
            />
          )}
        </div>
        {Boolean(r.sources?.length) && (
          <button className="link" onClick={() => show("share")}>
            <Link2 size={14} />
            {t(
              "他のプロジェクトから参照",
              "Link an existing credential",
              "引用其他项目的凭据",
            )}
          </button>
        )}
        {linked && (
          <button className="link" onClick={() => show("unlink")}>
            <Unlink size={14} />
            {t("参照を解除", "Unlink credential", "取消引用")}
          </button>
        )}
        {r.expires && (
          <small>
            {t("有効期限", "Expires", "到期")} · {r.expires}
          </small>
        )}
        {r.present && !linked && d.guide !== "apple-account" && (
          <button
            className="link cert"
            onClick={() =>
              users.length ? show("certificate") : action("certificate")
            }
          >
            <FileBadge size={13} />
            {t(
              "関連証明書の期限を取得",
              "Read related X.509 certificate",
              "读取关联证书有效期",
            )}
          </button>
        )}
        {r.present &&
          !linked &&
          d.kind === "secret" &&
          d.guide !== "apple-account" && (
            <button className="link cert" onClick={() => show("key-match")}>
              <ShieldCheck size={14} />
              {t(
                "証明書と秘密鍵を照合",
                "Match certificate to private key",
                "核对证书和私钥",
              )}
            </button>
          )}
        {r.fingerprint && (
          <small>
            {t(
              "秘密鍵と一致した証明書",
              "Certificate matching this key",
              "与私钥匹配的证书",
            )}{" "}
            · SHA-256 <code>{r.fingerprint}</code>
          </small>
        )}
        {r.evidence && <small>{r.evidence}</small>}
      </section>
      {d.key !== "APPLE_ACCOUNT_PASSWORD" && (
        <section>
          <h3>GitHub Actions</h3>
          <button disabled={!r.present} onClick={() => show("github")}>
            <RefreshCw size={14} />
            {d.kind === "secret"
              ? t("Secretに登録", "Set secret", "设置机密")
              : t("Variableに登録", "Set variable", "设置变量")}
          </button>
          {p.snap.githubApp && r.present && (
            <button onClick={() => show("app-inspect")}>
              <Search size={14} />
              {t("照会先を選択", "Choose lookup target", "选择查询目标")}
            </button>
          )}
          {r.synced && (
            <small>
              {t("最終送信", "Last sent", "上次发送")} · {r.synced}
            </small>
          )}
          {r.binding && (
            <div className="stack">
              <small>
                {r.binding.repo}
                {r.binding.environment && " / " + r.binding.environment}
              </small>
              <button onClick={() => action("remote")}>
                <RefreshCw size={13} />
                {t(
                  "登録日時を取得",
                  "Check remote timestamp",
                  "检查远端时间戳",
                )}
              </button>
              <label>
                <input
                  type="checkbox"
                  checked={r.binding.automatic}
                  onChange={() => action("watch")}
                />
                {t("15分ごとに確認", "Check every 15 min", "每 15 分钟检查")}
              </label>
              {r.binding.polled && (
                <small>
                  {t("確認結果", "Remote metadata", "远端元数据")} ·{" "}
                  {r.binding.remote === "unavailable"
                    ? t("取得失敗", "Unavailable", "获取失败")
                    : r.binding.remote || t("未登録", "Not found", "未找到")}
                </small>
              )}
            </div>
          )}
        </section>
      )}
      <section>
        <h3>{t("項目の意味と用途", "What this is used for", "含义与用途")}</h3>
        <p className="item-purpose">
          {item
            ? item.purpose[languageIndex[lang]]
            : t(
                "このプロジェクトに追加した独自項目です。用途と取得先は登録した担当者に確認してください。",
                "This is a project-specific credential. Ask the person who added it about its purpose and source.",
                "这是本项目自定义的凭据。请向添加者确认其用途与获取方式。",
              )}
        </p>
        {set && peers.length > 0 && (
          <div className="set-peers">
            <small>
              {t("同じセット", "In this set", "同组凭据")} · {t(...set.name)}
            </small>
            {peers.map(({ def }) => (
              <button
                key={def.key}
                type="button"
                className="link"
                onClick={() => p.select(p.snap.scope, def.key)}
              >
                <Link2 size={13} /> {credentialName(def, t)}
              </button>
            ))}
          </div>
        )}
      </section>
      <section>
        <h3>{t("取得ガイド", "Setup guide", "获取指南")}</h3>
        <ol>
          {steps.map((x, i) => (
            <li key={x}>
              <b>{i + 1}</b>
              {x}
            </li>
          ))}
        </ol>
        {links && (
          <div className="guide-links">
            <a
              href={links.docs}
              onClick={(e) => {
                e.preventDefault();
                action("source");
              }}
            >
              <ExternalLink size={13} />
              {t("公式ドキュメント", "Official documentation", "官方文档")}
            </a>
            <a
              href={links.get}
              onClick={(e) => {
                e.preventDefault();
                action("acquire");
              }}
            >
              <ExternalLink size={13} />
              {links.local
                ? t(
                    "作成・確認手順",
                    "Creation and lookup instructions",
                    "创建与查看步骤",
                  )
                : t(
                    "取得・管理ページ",
                    "Open management page",
                    "获取与管理页面",
                  )}
            </a>
          </div>
        )}
      </section>
      {d.group === "Tauri" && (
        <section>
          <h3>Tauri Updater</h3>
          <div className="stack">
            <button onClick={() => show("generate")}>
              <KeyRound size={14} />
              {t("新規鍵ペアを生成", "Generate key pair", "生成新密钥对")}
            </button>
            <button onClick={() => action("check-conf")}>
              <SlidersHorizontal size={14} />
              {t("設定ファイルと照合", "Check configuration", "检查配置文件")}
            </button>
            <button onClick={() => show("manifest")}>
              <Package size={14} />
              {t("更新JSONを生成", "Create update manifest", "生成更新清单")}
            </button>
            <button onClick={() => action("workflow")}>
              <Download size={14} />
              {t(
                "ビルド用Actionsを書き出す",
                "Export build workflow",
                "导出构建工作流",
              )}
            </button>
            <button onClick={() => action("verify")}>
              <ShieldCheck size={14} />
              {t(
                "署名 → 成果物を検証",
                "Verify signature → artifact",
                "验证签名 → 产物",
              )}
            </button>
          </div>
        </section>
      )}
    </aside>
  );
}
