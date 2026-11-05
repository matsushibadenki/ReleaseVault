# ReleaseVault

Tauri 2 / Rust / React / TypeScript のローカル署名情報管理アプリ。Android、iOS、iPadOS、Windows、macOS、Linuxの**配布先**を選ぶと、対応する項目のシートを表示します。macOSのGitHub配布とMac App Store、AndroidのPlayアップロード鍵と直接配布鍵、Windows StoreのMSIXとEXE/MSIを区別します。収録項目は候補で、すべてが必須という意味ではありません。

## 起動

今回のmacOS Apple Silicon用ビルドは `target/release/bundle/macos/ReleaseVault.app`。初回に12文字以上のマスターパスワードを設定します。本番配布用のDeveloper ID署名・公証は行っていません。

macOSの実行対象は10.15以降です。Macの認証を使う場合はSecure Enclave対応機が必要です。macOS版のビルドにはXcodeのSwiftコンパイラ、証明書処理用のOpenSSLビルドにはPerlとCツールチェーンが必要です。

開発にはNode.js 22.12以降、npm、Rust stableおよび[TauriのOS別前提条件](https://v2.tauri.app/start/prerequisites/)が必要です。

```sh
npm ci
npm run tauri -- dev
```

`npm run dev` 単独ではWebView用の配信サーバーのみ起動し、保管庫は利用できません。ビルドするには `npm run tauri -- build`。通常の出力先は `src-tauri/target/release/bundle/`（CARGO_TARGET_DIR指定時はその下のrelease/bundle/）。

## 操作

1. サイドバーの「＋」からプロジェクトと配布先を追加します。「プロジェクト設定」で編集できます。配布先を外しても値を消さず、戻すと再表示されます。
2. 配布先とDevelopment / Staging / Productionを選択。行をクリックすると取得手順と公式リンクを表示します。番号は作業の順序で、外部サービスの画面操作を代行するものではありません。
3. 値を入力するか、ファイルをネイティブダイアログから読み込みます。テキストとBase64を選択できます。秘密値は通常の一覧取得には含まれません。目のアイコンで再認証すると15秒だけ表示します。
4. 状態・名前・キー名で絞り込み、項目名の見出しで並べ替え。⌘/CtrlクリックまたはShiftクリックで複数選択し、⌘/Ctrl+Cで名前・キー名・状態をコピーできます。取得ガイドは右上のボタンで別ウインドウに分離できます。
5. X.509証明書（DER/PEM）を選ぶとSubject・シリアル番号・期限を関連付けます。30日以内と期限切れを区別します。関連付けは証明書と秘密鍵の一致保証ではありません。

### Apple IDで認証する場合

Appleの配布先で「Apple Account認証」セットを開きます。`APPLE_ID` はサインイン用メールアドレス、`APPLE_ACCOUNT_PASSWORD` は普段Apple Accountにサインインするときのパスワード、`APPLE_PASSWORD` は公証やアップロードに使うアプリ用パスワードです。3項目とも暗号化して保管します。GitHub Actionsに登録できるのは、このうちメールアドレスとアプリ用パスワードです。[Appleの発行手順](https://support.apple.com/102654)

macOS向けに書き出すビルド用ActionsでApple IDによる公証を行う場合は、Team IDを `APPLE_TEAM_ID` のVariableに登録します。App Store Connectの.p8を設定した場合はAPIキー方式を使用します。

### 証明書と秘密鍵の照合

秘密鍵を保存した行で「証明書と秘密鍵を照合」を押し、鍵ファイルにパスワードがある場合は入力して、照合する公開証明書（.cer / .der / .pem / .crt）を選びます。.p12 / .pfx はBase64、PEM秘密鍵はテキスト、DER秘密鍵はBase64で読み込みます。Rustが両者の公開鍵を比較し、成功時に証明書のSHA-256指紋・Subject・シリアル番号・期限を記録します。「一致」は選択した証明書との鍵の対応を示します。失効や発行元の信頼性は別途確認してください。

### プロジェクト間の共有

受け取り側で同じ環境の未登録項目を選び、「他のプロジェクトから参照」で共有元を指定して再認証します。同じキー名・種類の保存済み項目が候補になります。Updaterの鍵と設定は各アプリで独立して管理します。

編集は共有元で行います。変更前に影響を受けるプロジェクトを確認して再認証すると、参照先にも新しい値が反映されます。値を差し替えた後のGitHub登録は、各登録先を確認して再送信してください。共有をやめるには受け取り側で「参照を解除」を選びます。

### Macの認証で解除

対応するMacで保管庫を開き、サイドバーの「Macの認証設定」から有効にします。初回はマスターパスワードを入力し、macOSの本人確認を完了してください。次回からロック画面に「Macの認証で解除」が表示されます。同じ設定から無効化できます。

解除用の記録はこの端末のSecure Enclaveに結び付きます。別端末への復元と、秘密値の一時表示・GitHubへの送信などの再認証にはマスターパスワードを使います。

### パスキーで解除（macOS 15以降）

Associated Domainsを設定した署名済みビルドで、サイドバーの「パスキー設定」を選びます。マスターパスワードを入力し、macOSでパスキーを作成した後、同じパスキーで本人確認すると登録が完了します。次回から「パスキーで解除」を選択できます。PRFに対応したパスキープロバイダが必要です。復元先ではマスターパスワードで保管庫を開き、改めて登録します。「パスキー設定」から解除を無効にしても、プロバイダ側のパスキーは自動削除されません。

配布担当者向け：Apple DeveloperでこのアプリのApp IDにAssociated Domainsを有効化し、該当するmacOSプロビジョニングプロファイルと署名IDを用意します。以下をリポジトリ直下で実行すると、配布用設定と公開用の関連付けファイルを生成できます。

```sh
python3 scripts/prepare-passkeys.py <管理下のホスト名> <プロファイルのパス> '<署名ID>'
npm run tauri -- build --bundles app --config target/passkeys/<管理下のホスト名>/tauri.passkeys.json
```

生成された `.well-known/apple-app-site-association` のみを、指定ホストの同じパスでHTTPS公開します（Content-Type: application/json、リダイレクトなし）。既存の関連付けがある場合はwebcredentials.appsにApp IDを追加します。保管庫や秘密値のアップロードは不要です。設定のない通常ビルドは従来の解除方法を使います。[Appleの設定手順](https://developer.apple.com/documentation/authenticationservices/connecting-to-a-service-with-passkeys)

### GitHub Appで接続する

1. GitHubの個人または組織のDeveloper settingsで、自分が管理する非公開のGitHub Appを作成します。Webhookは使いません。Repository permissionsで、対象に応じてSecrets、Variables、Environmentsを許可します。登録する種類にはRead and write、監視だけならReadを選びます。
2. Appを対象アカウントにインストールし、Only select repositoriesで対象を選びます。App設定でApp IDと秘密鍵ファイル（.pem）を取得します。Installation IDはインストール設定のURL末尾にある数字です。
3. ReleaseVaultで対象プロジェクトを開き、サイドバーの **GitHub App** に両方のIDとマスターパスワードを入力して.pemを選びます。鍵はRustで読み込み、ローカル保管庫とそのバックアップに暗号化して保存します。秘密鍵はそのAppのほかのインストールに対する認証にも使えるため、自分で管理する専用Appを使用してください。
4. **GitHub App** を再度開き、リポジトリを選びます。Repository Secrets、Repository Variables、EnvironmentのSecrets・Variablesごとに許可を保存します。新規リポジトリはすべて「許可しない」で開始します。GitHub側の対象や権限を変えたら再取得ボタンを押します。
5. 項目の「Secretに登録」「Variableに登録」で送信先を選びます。Environment欄が空ならリポジトリ直下、入力した場合はそのEnvironmentが対象です。送信時には対象リポジトリ1件と操作に必要な権限に絞った短期トークンを取得します。SecretはGitHubの対象公開鍵で暗号化して送ります。読み取りだけを許可した場合は、保存済みの行で「照会先を選択」から更新日時を取得し、必要に応じて15分ごとの監視を有効にします。照会と監視には読み取りトークンを使用します。

接続解除は、このプロジェクトの接続鍵・権限設定を削除して監視を停止します。GitHub側のインストールや発行済み秘密鍵の失効はGitHubで管理します。接続中のプロジェクトでは、送信と監視に設定したGitHub Appを使用します。対象はGitHub.comの個人・組織インストールです。[GitHubのインストールトークン](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app)

### GitHub CLIで接続する（App未接続のプロジェクト）

[GitHub CLI](https://cli.github.com/manual/gh_auth_login)をインストールし、ターミナルで `gh auth login --hostname github.com --web` を実行します。対象リポジトリのSecretsまたはVariablesの書込権限が必要です。

保存済みの行で登録先のowner/repo・Environment・キー名を確認し、再認証して送信します。Environmentが空ならリポジトリ単位です。Rustが値を標準入力で公式CLIに渡します。[GitHub CLIのSecret登録](https://cli.github.com/manual/gh_secret_set)がクライアント側暗号化を担当します。アプリはGitHubトークンを保存しません。機密型はSecrets、それ以外はVariablesです。送信上限は48 KiB。

成功後の「登録日時を取得」はリモートの存在・更新日時だけを取得します。監視を有効にした項目は、起動・ロック解除中に約15分間隔で再確認します（1分に最大1項目）。GitHubからSecretの値を読み戻したり、保存した値との一致や公開成功を報告したりはしません。

### Tauri Updater

直接更新を扱う配布先でTauriの行を選択し、鍵ペア・専用パスワードを生成します。既存鍵は上書きしません。対象アプリの設定に公開鍵とHTTPSエンドポイントを追加し、平文JSONの `tauri.conf.json` を選択して照合します。既に配布したアプリの鍵交換には移行計画が必要です。

署名検証は `.sig` → 成果物の順で選択。成果物はRustでストリーム検証します。更新JSON生成には、入力した版と署名内の版が一致する必要があります（Tauri CLIで手動署名する場合は `--app-version`）。生成する `latest.json` は指定したプラットフォーム1件用です。公開URLの到達性・サーバー側ファイルの内容は未確認です。

「ビルド用Actionsを書き出す」は選択したデスクトップOS向けの手動実行ワークフローを生成します。対象リポジトリでnpm・Tauri CLI・Updater設定、GitHub Environmentと必要な資格情報を準備し、内容を確認して配置してください。更新用署名とOSのコード署名は別です。特にWindowsの署名プロバイダー設定は別途必要です。自動コミット・自動公開はしません。参考: [Tauri Updater](https://v2.tauri.app/plugin/updater/)。

### CSVとバックアップ

CSVはUTF-8、ヘッダーは次の順序です。引用符・改行を含むCSVに対応し、一行でも無効なら全体を取り消します。既登録値の上書きは拒否します。エクスポートでは通常値を含め**すべての値を除外**します。

```csv
key,name,kind,value,expires
RELEASE_ACCOUNT,Release account,id,example-team,
CUSTOM_SIGNING_TOKEN,Signing token,secret,,2028-12-31
```

復旧にはCSVではなく「バックアップ」で作成する暗号化済み `.rvbackup` とマスターパスワードの両方が必要です。新しい保管庫の初回画面から復元できます。復元は既存の保管庫を上書きしません。別フォルダーで復元を試すには、アプリを終了し、macOSでは以下のように起動します。

```sh
open -n "/path/to/ReleaseVault.app" --args --vault-dir "/absolute/path/to/new-vault"
```

## 保管方式と検証

Rustがドキュメント・選択状態・鍵を所有し、ウインドウには表示用データだけを渡します。SQLiteの1レコードを変更単位とし、暗号化した変化をリビジョン付きで保存します。OS乱数で生成した256ビット鍵をArgon2id（64 MiB、3回）由来の鍵でラップし、ドキュメントと各値をXChaCha20-Poly1305で暗号化します。バックアップは端末のキーチェーンに依存しません。5分の無操作でメモリ上のセッションを破棄します。アプリが扱うのはローカルファイルと利用者が指定したGitHub登録先であり、専用サーバーは不要です。

```sh
npm ci
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

テストは実際のTauri CLIによる署名と改変拒否、マニフェストの版・配布先、暗号化保存と復元、環境分離、CSVの原子性、模擬CLIでのGitHub送信、ローカル模擬REST APIでのJWT署名・権限制限・Secret暗号化・Variable登録・メタデータ監視を検査します。外部アカウントは不要です。

## Roadmap

- 🟢 [Done] 3言語のシート、14の配布先、独自項目、期限・監査履歴、パネル分離、暗号化Vault、ロック、CSV、暗号化バックアップ。
- 🟢 [Done] GitHub CLI経由のSecret・Variable登録とメタデータ監視、Updater鍵生成・設定照合・署名検証・単一プラットフォームの更新JSON、ビルドワークフロー書き出し。
- 🟢 [Done] 証明書と保存済み秘密鍵の公開鍵照合、指紋記録、プロジェクト間の参照共有、変更前の影響先表示、対応Macの本人認証によるロック解除。
- 🟢 [Done] GitHub App接続、リポジトリ単位の権限設定、対象を限定したトークンでの登録と監視。2026年11月5日に、実際のGitHub Appと非公開リポジトリ1件でRust側の接続・権限切り替え・SecretとVariableの作成および更新を確認。読み取り権限で取得した更新日時はGitHub CLIでの照会と一致。
- 🔴 [Later] Apple / Google / MicrosoftのアカウントAPI、自動更新・失効手続き、Actions結果の取込、ワークフロー変更PR、HSM/KMS・チーム承認・端末間E2EE同期。
- ⭕️ [Pending] 本人操作を伴うMac認証の実機確認、実リポジトリでのActionsビルド、ストアへの提出、Windows/Linuxでの実機動作、配布用macOS署名・公証。
