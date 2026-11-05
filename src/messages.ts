import type { T } from "./types";
export function errorText(code: string, t: T) {
  switch (code) {
    case "vault-only":
      return t(
        "このパスワードは保管庫でのみ管理します。",
        "This password is managed only in the vault.",
        "此密码仅在保险库中管理。",
      );
    case "app-key":
      return t(
        "GitHub AppのRSA秘密鍵（.pem）を選んでください。",
        "Choose the GitHub App RSA private key (.pem).",
        "请选择 GitHub App 的 RSA 私钥（.pem）。",
      );
    case "app-auth":
      return t(
        "App ID、Installation ID、秘密鍵、接続状態を確認してください。",
        "Check the App ID, installation ID, key and installation status.",
        "请检查 App ID、安装 ID、私钥和安装状态。",
      );
    case "app-policy":
      return t(
        "この操作にはリポジトリの権限が必要です。設定を確認してください。",
        "Check the repository grant for this operation.",
        "请检查此操作所需的仓库权限。",
      );
    case "app-missing":
      return t(
        "GitHubで対象を確認できません。対象と権限を確認してください。",
        "GitHub could not find the target. Check its name and access.",
        "GitHub 无法找到目标。请检查名称与权限。",
      );
    case "app-grant":
    case "app-denied":
      return t(
        "GitHubが要求を拒否しました。権限・入力内容・利用制限を確認してください。",
        "GitHub declined the request. Check permissions, input and rate limits.",
        "GitHub 已拒绝请求。请检查权限、输入与速率限制。",
      );
    case "app-network":
      return t(
        "GitHubと通信できません。接続を確認してください。",
        "Could not reach GitHub. Check your connection.",
        "无法连接 GitHub。请检查网络。",
      );
    case "app-api":
      return t(
        "GitHubから予期しない応答がありました。",
        "GitHub returned an unexpected response.",
        "GitHub 返回了意外响应。",
      );
    case "app-config":
      return t(
        "GitHub Appの接続設定を確認し、画面を開き直してください。",
        "Check the GitHub App connection and reopen this dialog.",
        "请检查 GitHub App 连接并重新打开此对话框。",
      );
    case "passkey":
      return t(
        "パスキー認証を完了できませんでした。PRF対応のパスキーで再試行するか、マスターパスワードを使用してください。",
        "Passkey verification did not finish. Retry with a PRF-compatible passkey or use your master password.",
        "未能完成通行密钥验证。请使用支持 PRF 的通行密钥重试，或使用主密码。",
      );
    case "device":
      return t(
        "Macの認証が完了しませんでした。マスターパスワードでも解除できます。",
        "Mac authentication did not complete. You can use your master password.",
        "Mac 身份验证未完成，也可以使用主密码解锁。",
      );
    case "share":
    case "impact":
      return t(
        "共有関係が変わりました。対象プロジェクトを再確認してください。",
        "Review the current linked projects and try again.",
        "请重新检查引用项目后再试。",
      );
    case "shared-readonly":
      return t(
        "この値は共有元で編集してください。",
        "Edit this value in the source project.",
        "请在来源项目中编辑此值。",
      );
    case "shared-in-use":
      return t(
        "値を削除する前に共有先の参照を解除してください。",
        "Unlink the dependent projects before clearing this value.",
        "请先取消其他项目的引用再清除。",
      );
    case "key-format":
      return t(
        "保存した秘密鍵の形式（PKCS#12、PEM、DER）と鍵ファイルのパスワードを確認してください。",
        "Check the stored private key format (PKCS#12, PEM, DER) and its file password.",
        "请检查私钥格式（PKCS#12、PEM、DER）及文件密码。",
      );
    case "key-mismatch":
      return t(
        "選択した証明書と保存済みの秘密鍵が一致しません。",
        "The certificate does not match the stored private key.",
        "所选证书与保存的私钥不匹配。",
      );
    case "version":
      return t(
        "署名内のバージョンが一致しません。--app-version を指定して署名してください。",
        "Sign with --app-version matching the manifest version.",
        "请使用与清单版本一致的 --app-version 签名。",
      );
    case "auth":
      return t(
        "パスワードが違うか、データが破損しています。",
        "Incorrect password or damaged vault.",
        "密码错误或保险库已损坏。",
      );
    case "weak":
      return t(
        "12文字以上のパスワードを入力してください。",
        "Use at least 12 characters.",
        "请输入至少 12 个字符。",
      );
    case "occupied":
      return t(
        "既存の値は上書きしません。対象項目を確認してください。",
        "Existing values will not be overwritten. Check the selected items.",
        "不会覆盖已有值。请检查所选项目。",
      );
    case "gh":
      return t(
        "GitHub CLIのインストール、gh auth login、登録先の権限を確認してください。",
        "Check that GitHub CLI is installed, signed in, and authorized for this destination.",
        "请确认 GitHub CLI 已安装、已登录且有目标仓库权限。",
      );
    case "limit":
      return t(
        "GitHubの送信上限は48 KBです。",
        "GitHub values must fit within 48 KB.",
        "GitHub 值大小不能超过 48 KB。",
      );
    case "csv":
      return t(
        "CSVの列は key,name,kind,value,expires の順で指定します。",
        "Expected CSV header: key,name,kind,value,expires.",
        "CSV 列顺序应为 key,name,kind,value,expires。",
      );
    case "mismatch":
      return t(
        "公開鍵・署名またはエンドポイントが一致しません。",
        "Public key, signature, or endpoint did not match.",
        "公钥、签名或端点不匹配。",
      );
    case "destination":
      return t(
        "登録先の形式を確認してください。例: owner/repo",
        "Check the destination. Example: owner/repo",
        "请检查目标格式，例如 owner/repo。",
      );
    case "duplicate":
      return t(
        "このキー名は既に登録されています。",
        "This key name already exists.",
        "此密钥名称已存在。",
      );
    case "locked":
      return t(
        "保管庫はロックされています。",
        "The vault is locked.",
        "保险库已锁定。",
      );
    case "missing":
      return t(
        "必要な値が未登録です。",
        "A required value is missing.",
        "缺少必需的值。",
      );
    case "busy":
      return t("処理中です。", "Operation in progress.", "正在处理。");
    case "scope":
    case "conflict":
      return t(
        "他の画面で内容が変わりました。再選択してください。",
        "The workspace changed. Select the item again.",
        "工作区已更改。请重新选择项目。",
      );
    case "size":
      return t(
        "値が空か、サイズの上限を超えています。",
        "The value is empty or too large.",
        "值为空或超出大小限制。",
      );
    case "format":
      return t(
        "ファイル形式を確認してください。",
        "Check the file format.",
        "请检查文件格式。",
      );
    default:
      return (
        t(
          "操作を完了できませんでした。入力と保存先を確認してください。",
          "Could not complete the operation. Check the input and destination.",
          "操作未完成。请检查输入和保存位置。",
        ) +
        " (" +
        code.slice(0, 48) +
        ")"
      );
  }
}
