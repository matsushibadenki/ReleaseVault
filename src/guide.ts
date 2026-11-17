export type Lang = "ja" | "en" | "zh";
export type Words = [string, string, string];
export const translate = (l: Lang) => (ja: string, en: string, zh: string) =>
  [ja, en, zh][["ja", "en", "zh"].indexOf(l)];
export const targets: Record<string, string> = {
  "mac-direct": "macOS · GitHub Releases",
  "mac-store": "macOS · Mac App Store",
  ios: "iOS · App Store",
  ipados: "iPadOS · App Store",
  "android-play": "Android · Google Play",
  "android-apk": "Android · APK",
  "windows-direct": "Windows · GitHub Releases",
  "windows-msix": "Windows · Store (MSIX)",
  "windows-exe": "Windows · Store (EXE/MSI)",
  "linux-appimage": "Linux · AppImage",
  "linux-deb": "Linux · DEB",
  "linux-rpm": "Linux · RPM",
  "linux-flatpak": "Linux · Flathub",
  "linux-snap": "Linux · Snap Store",
};
type Three = [Words, Words, Words];
const guides: Record<string, Three> = {
  "developer-id": [
    [
      "キーチェーンアクセスで署名要求（CSR）を作成します。",
      "Create a certificate signing request in Keychain Access.",
      "在钥匙串访问中创建证书签名请求（CSR）。",
    ],
    [
      "Apple DeveloperでDeveloper ID Applicationを選び、CSRを提出します。",
      "Select Developer ID Application in Apple Developer and submit the CSR.",
      "在 Apple Developer 中选择 Developer ID Application 并提交 CSR。",
    ],
    [
      "発行された証明書を読み込み、対応する秘密鍵と一緒に.p12へ書き出します。",
      "Install the certificate and export it with its matching private key as a .p12.",
      "导入证书，并将其与匹配的私钥一起导出为 .p12。",
    ],
  ],
  "apple-account": [
    [
      "Apple Developerに参加しているApple AccountのメールアドレスをApple IDに入力します。",
      "Enter the email address of your Apple Developer account under Apple ID.",
      "在 Apple ID 栏输入加入 Apple Developer 的 Apple 账户邮箱。",
    ],
    [
      "2ファクタ認証を有効にし、account.apple.comの「サインインとセキュリティ」でアプリ用パスワードを生成します。",
      "Enable two-factor authentication, then generate an app-specific password under Sign-In and Security at account.apple.com.",
      "启用双重认证，然后在 account.apple.com 的“登录与安全”中生成 App 专用密码。",
    ],
    [
      "発行したパスワードを「Apple Accountのアプリ用パスワード」に保存します。macOSの公証ではTeam IDも指定します。",
      "Save the generated password under Apple app-specific password. For macOS notarization, also enter the Team ID.",
      "将生成的密码保存到“Apple App 专用密码”。macOS 公证还需填写团队 ID。",
    ],
  ],
  "apple-id": [
    [
      "Apple Developerで対象チームを選びます。",
      "Select your team in Apple Developer.",
      "在 Apple Developer 中选择团队。",
    ],
    [
      "Team IDはメンバーシップ詳細、Bundle IDはIdentifiersで確認します。",
      "Find the Team ID in membership details and the Bundle ID under Identifiers.",
      "在会员信息中查找 Team ID，在 Identifiers 中查找 Bundle ID。",
    ],
    [
      "ビルド設定の署名チーム・識別子と照合して保存します。",
      "Match the signing team and identifier to your build settings, then save.",
      "与构建配置的签名团队和标识符核对后保存。",
    ],
  ],
  asc: [
    [
      "App Store Connectの「ユーザとアクセス」で統合を開きます。",
      "Open Integrations under Users and Access in App Store Connect.",
      "在 App Store Connect 的用户和访问中打开集成。",
    ],
    [
      "必要な権限のチームAPIキーを発行し、Key IDとIssuer IDを控えます。",
      "Create a team API key with the required role; record its Key ID and Issuer ID.",
      "按所需权限创建团队 API 密钥并记录 Key ID 和 Issuer ID。",
    ],
    [
      "一度だけ取得できる.p8はテキストとして読み込み、オフラインにも保管します。",
      "Import the one-time .p8 download as text and keep an offline backup.",
      "将仅可下载一次的 .p8 作为文本导入，并进行离线备份。",
    ],
  ],
  "apple-store": [
    [
      "対象のApp IDに、アプリが使用する機能を登録します。",
      "Register the capabilities your application uses on its App ID.",
      "为 App ID 注册应用使用的功能。",
    ],
    [
      "配布用証明書を作成し、App IDと証明書に対応するストア用プロファイルを取得します。",
      "Create a distribution certificate and a store profile for the App ID and certificate.",
      "创建分发证书，并生成与 App ID 和证书匹配的商店描述文件。",
    ],
    [
      "Xcodeの署名設定と照合します。MacのPKG提出時はインストーラ署名も用意します。",
      "Check Xcode signing settings. For Mac PKG submission, also prepare an installer identity.",
      "核对 Xcode 签名设置。提交 Mac PKG 时还需准备安装包签名。",
    ],
  ],
  android: [
    [
      "Android Studioの署名済みバンドル／APKの生成を開きます。",
      "Open Generate Signed Bundle / APK in Android Studio.",
      "在 Android Studio 中打开生成签名的 Bundle / APK。",
    ],
    [
      "配布先のキーストアを作成または選択し、エイリアスとパスワードを記録します。",
      "Create or select the keystore for this channel and record its alias and passwords.",
      "创建或选择该渠道的密钥库，记录别名和密码。",
    ],
    [
      "JKSはBase64で読み込みます。Playにはアップロード鍵を使用します。",
      "Import the JKS as Base64. For Play, use the upload key.",
      "以 Base64 导入 JKS。发布到 Play 时使用上传密钥。",
    ],
  ],
  play: [
    [
      "Play Consoleで対象アプリを選びます。",
      "Select the app in Play Console.",
      "在 Play 管理中心选择应用。",
    ],
    [
      "アプリの完全性で署名証明書の指紋を確認します。API利用時はサービスアカウントに権限を付与します。",
      "Find the signing fingerprint in App integrity. For API access, grant the service account app permissions.",
      "在应用完整性页面查看证书指纹。使用 API 时为服务账号授予应用权限。",
    ],
    [
      "公開トラックとアプリのパッケージ名を照合します。",
      "Match the release track and package name to the app.",
      "核对应用的发布轨道与包名。",
    ],
  ],
  windows: [
    [
      "使用する署名方式（証明書・HSM・クラウド署名）を選びます。",
      "Choose certificate, hardware, or cloud signing.",
      "选择证书、硬件或云签名。",
    ],
    [
      "証明書の拇印、またはクラウド署名プロファイルを確認します。",
      "Find the certificate thumbprint or cloud signing profile.",
      "查找证书指纹或云签名配置文件。",
    ],
    [
      "発行者をパッケージのPublisherと照合し、識別子を保存します。",
      "Match the issuer identity to the package publisher and save the identifier.",
      "核对软件包发布者后保存标识符。",
    ],
  ],
  microsoft: [
    [
      "Partner Centerで製品名を予約します。",
      "Reserve the product name in Partner Center.",
      "在合作伙伴中心预留产品名称。",
    ],
    [
      "製品のID情報からPublisherとPackage Identityを取得します。",
      "Copy the publisher and package identity from product identity details.",
      "从产品标识页复制发布者和包标识。",
    ],
    [
      "MSIXとEXE/MSIの提出先を分けて保存します。",
      "Keep MSIX and EXE/MSI submission targets separate.",
      "分别保存 MSIX 和 EXE/MSI 提交目标。",
    ],
  ],
  gpg: [
    [
      "配布用のGPG鍵を生成するか、既存の鍵を選びます。",
      "Generate a distribution GPG key or select an existing one.",
      "生成分发用 GPG 密钥或选择已有密钥。",
    ],
    [
      "gpg --list-secret-keys --fingerprint で指紋を確認します。",
      "Check its fingerprint with gpg --list-secret-keys --fingerprint.",
      "使用 gpg --list-secret-keys --fingerprint 核对指纹。",
    ],
    [
      "用途と指紋を照合し、必要な場合だけ署名用の秘密鍵を読み込みます。",
      "Match the fingerprint and purpose; import secret key material only if needed.",
      "核对用途与指纹，仅在需要时导入签名私钥。",
    ],
  ],
  flatpak: [
    [
      "Flathubに登録するアプリのドメインを確認します。",
      "Confirm the domain for the Flathub application.",
      "确认 Flathub 应用的域名。",
    ],
    [
      "マニフェストのapp-idを確認します。",
      "Read the app-id from the Flatpak manifest.",
      "从 Flatpak 清单读取 app-id。",
    ],
    [
      "公開申請のIDと照合して保存します。",
      "Match it to the submission ID and save.",
      "与提交 ID 核对后保存。",
    ],
  ],
  snap: [
    [
      "Snap Storeにログインして対象のSnapを選びます。",
      "Sign in to the Snap Store and select your snap.",
      "登录 Snap Store 并选择目标 Snap。",
    ],
    [
      "snapcraft export-loginで対象のSnapと権限を制限します。",
      "Use snapcraft export-login with restricted snaps and permissions.",
      "使用 snapcraft export-login 限制 Snap 和权限。",
    ],
    [
      "認証ファイルをテキストとして読み込み、期限を設定します。",
      "Import the credential file as text and set its expiry.",
      "将凭据文件作为文本导入并设置到期日期。",
    ],
  ],
  updater: [
    [
      "直接配布版のプロジェクトと環境を選びます。",
      "Select the project and environment for direct distribution.",
      "选择直接分发的项目和环境。",
    ],
    [
      "更新鍵を生成するか既存の鍵を読み込みます。配布済みアプリの鍵は保持してください。",
      "Generate or import an updater key pair. Keep keys used by shipped versions.",
      "生成或导入更新密钥对。请保留已发布版本使用的密钥。",
    ],
    [
      "公開鍵とHTTPSエンドポイントをtauri.conf.jsonに設定し、照合します。",
      "Set the public key and HTTPS endpoint in tauri.conf.json, then check the configuration.",
      "在 tauri.conf.json 中设置公钥和 HTTPS 端点，然后检查配置。",
    ],
  ],
  github: [
    [
      "GitHub Appを接続して権限を設定するか、gh auth login でログインします。",
      "Connect a GitHub App and grant repository access, or sign in with gh auth login.",
      "连接 GitHub App 并设置仓库权限，或使用 gh auth login 登录。",
    ],
    [
      "対象リポジトリのowner/repo名を確認します。",
      "Check the repository owner/name on GitHub.",
      "在 GitHub 上确认 owner/repo 名称。",
    ],
    [
      "送信前にリポジトリ・Environment・キー名を確認します。",
      "Review the repository, environment, and key name before sending.",
      "发送前核对仓库、环境和密钥名称。",
    ],
  ],
  custom: [
    [
      "管理する情報の発行元を確認します。",
      "Identify the issuer of this value.",
      "确认此值的颁发方。",
    ],
    [
      "対象アプリと環境に合う値を用意します。",
      "Obtain the value for this app and environment.",
      "获取适用于此应用和环境的值。",
    ],
    [
      "値を保存し、有効期限があれば設定します。",
      "Save the value and set an expiry if applicable.",
      "保存并按需设置到期日期。",
    ],
  ],
};
export function guide(id: string, lang: Lang) {
  return (guides[id] ?? guides.custom).map((x) => translate(lang)(...x));
}

export function credentialName(
  def: { key: string; name: string; guide: string },
  t: ReturnType<typeof translate>,
) {
  if (def.key === "GITHUB_SSH_KEY_PATH" && def.guide === "github")
    return t("SSHキーのパス", "SSH key path", "SSH 密钥路径");
  if (def.guide === "apple-id" && def.key === "APPLE_APP_ID")
    return t(
      "App ID（アプリ識別子）",
      "App ID (app identifier)",
      "App ID（应用标识符）",
    );
  if (def.guide !== "apple-account") return def.name;
  if (def.key === "APPLE_ACCOUNT_PASSWORD")
    return t(
      "Apple Accountのサインイン用パスワード",
      "Apple Account sign-in password",
      "Apple 账户登录密码",
    );
  return def.key === "APPLE_ID"
    ? t("Apple ID（メールアドレス）", "Apple ID (email)", "Apple ID（邮箱）")
    : t(
        "Apple Accountのアプリ用パスワード",
        "Apple Account app-specific password",
        "Apple 账户的 App 专用密码",
      );
}
