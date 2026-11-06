import type { T } from "./types";
export function errorText(code: string, t: T) {
  switch (code) {
    case "apple-config":
      return t(
        "同じプロジェクト・環境にApp Store ConnectのIssuer ID、Key ID、APIキー（.p8）を保存してください。",
        "Save the App Store Connect Issuer ID, Key ID and API key (.p8) in this project and environment.",
        "请在本项目和环境中保存 App Store Connect 的 Issuer ID、Key ID 和 API 密钥（.p8）。",
      );
    case "apple-key":
      return t(
        "App Store ConnectのP-256秘密鍵（.p8）を確認してください。",
        "Check the App Store Connect P-256 private key (.p8).",
        "请检查 App Store Connect 的 P-256 私钥（.p8）。",
      );
    case "apple-auth":
      return t(
        "AppleのAPIキーとIssuer ID・Key ID、有効状態、Macの時刻を確認してください。",
        "Check the API credentials, their validity and your Mac’s clock.",
        "请检查 API 凭据、有效状态及 Mac 的时间。",
      );
    case "apple-denied":
      return t(
        "このAPIキーには証明書・プロファイルを参照する権限がありません。",
        "This API key cannot read certificates or profiles.",
        "此 API 密钥无权读取证书或描述文件。",
      );
    case "apple-rate":
      return t(
        "Appleの取得回数制限に達しました。時間をおいて再試行してください。",
        "Apple rate limit reached. Try again later.",
        "已达到 Apple 请求限制，请稍后重试。",
      );
    case "apple-network":
      return t(
        "Appleから取得できませんでした。前回の取得結果を確認してください。",
        "Could not fetch from Apple. Previous results are retained.",
        "无法从 Apple 获取数据，已保留上次结果。",
      );
    case "apple-response":
      return t(
        "Appleの応答を確認できませんでした。期限は反映されていません。",
        "Could not validate Apple’s response. Expiry dates were not applied.",
        "无法验证 Apple 的响应，未更新到期日期。",
      );
    case "apple-limit":
      return t(
        "取得件数または時間が上限に達しました。期限は反映されていません。",
        "The fetch limit was reached. Expiry dates were not applied.",
        "获取数量或时间达到上限，未更新到期日期。",
      );
    case "apple-stale":
      return t(
        "APIキーまたは取得結果が変更されています。再取得してください。",
        "API credentials or results changed. Fetch them again.",
        "API 凭据或结果已更改，请重新获取。",
      );
    case "apple-file":
      return t(
        "保存ファイルと.p12の書き出し用パスワードを確認してください。",
        "Check the saved file and its .p12 export password.",
        "请检查保存的文件和 .p12 导出密码。",
      );
    case "apple-mismatch":
      return t(
        "選択したAppleの項目と保存ファイルが一致しません。別の項目を選択してください。",
        "The Apple resource does not match the saved file. Choose the matching resource.",
        "所选 Apple 项目与保存文件不匹配，请选择对应项目。",
      );
    case "workflow-stale":
      return t(
        "対象のブランチや接続が変更されています。差分を取り直してください。",
        "The branch or connection changed. Fetch a new preview.",
        "分支或连接已更改，请重新获取预览。",
      );
    case "workflow-file":
      return t(
        "既存のワークフローを読み取れません。UTF-8の通常ファイル（256 KiB以下）を確認してください。",
        "Check that the workflow is a UTF-8 file no larger than 256 KiB.",
        "请确认工作流是 256 KiB 以下的 UTF-8 普通文件。",
      );
    case "workflow-identical":
      return t(
        "既存ファイルと同じ内容です。",
        "The existing file already matches.",
        "内容与现有文件一致。",
      );
    case "workflow-denied":
      return t(
        "GitHubのContents・Workflows・Pull requests権限を確認してください。",
        "Check Contents, Workflows and Pull requests permissions on GitHub.",
        "请检查 GitHub 的 Contents、Workflows 和 Pull requests 权限。",
      );
    case "workflow-network":
      return t(
        "GitHubとの処理を完了できませんでした。PR作成中だった場合は、同じ差分画面から再試行すると既存ブランチ・PRを確認します。",
        "GitHub did not complete the request. Retry from the same preview to recover an existing branch or PR.",
        "GitHub 请求未完成。请从同一预览重试，以检查并恢复已有分支或 PR。",
      );
    case "release-version":
      return t(
        "追加するバージョンを更新JSONのまとめとそろえてください。別の版を作る場合は、既存のプラットフォームを削除してください。",
        "Match the draft version. Remove existing platforms to start another release.",
        "请使用与草稿一致的版本。创建其他版本前，请移除现有平台。",
      );
    case "release-duplicate":
      return t(
        "このプラットフォームは追加済みです。変更する場合は一覧から削除して再検証してください。",
        "Remove the existing platform before verifying its replacement.",
        "请先移除已有平台，再验证替换的产物。",
      );
    case "release-key":
      return t(
        "検証後に公開鍵が変更されています。該当するプラットフォームを削除して再検証してください。",
        "The public key has changed. Remove and verify the affected platform again.",
        "验证后公钥已更改。请移除并重新验证对应平台。",
      );
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
