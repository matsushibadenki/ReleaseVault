import type { Words } from "./guide";
import type { Def, Row, T } from "./types";

type CredentialSet = { name: Words; keys: string[] };

// Only list credentials with a shared purpose. Independent fields keep their provider heading.
const sets: CredentialSet[] = [
  {
    name: [
      "Apple Account認証",
      "Apple Account authentication",
      "Apple 账户认证",
    ],
    keys: ["APPLE_ID", "APPLE_ACCOUNT_PASSWORD", "APPLE_PASSWORD"],
  },
  {
    name: ["Appleのアプリ識別", "Apple app identification", "Apple 应用标识"],
    keys: ["APPLE_TEAM_ID", "APPLE_BUNDLE_ID", "APPLE_APP_ID"],
  },
  {
    name: [
      "App Store Connect API認証",
      "App Store Connect API authentication",
      "App Store Connect API 认证",
    ],
    keys: ["APPLE_API_ISSUER", "APPLE_API_KEY", "APPLE_API_KEY_P8"],
  },
  {
    name: [
      "Developer IDアプリ署名",
      "Developer ID application signing",
      "Developer ID 应用签名",
    ],
    keys: [
      "APPLE_CERTIFICATE",
      "APPLE_CERTIFICATE_PASSWORD",
      "APPLE_SIGNING_IDENTITY",
    ],
  },
  {
    name: [
      "ストア配布用証明書",
      "Store distribution certificate",
      "商店分发证书",
    ],
    keys: ["APPLE_STORE_CERTIFICATE", "APPLE_STORE_CERTIFICATE_PASSWORD"],
  },
  {
    name: [
      "ストア用インストーラ署名",
      "Store installer signing",
      "商店安装包签名",
    ],
    keys: ["APPLE_INSTALLER_CERTIFICATE", "APPLE_INSTALLER_PASSWORD"],
  },
  {
    name: ["APK署名鍵", "APK signing key", "APK 签名密钥"],
    keys: [
      "ANDROID_KEYSTORE",
      "ANDROID_KEY_ALIAS",
      "ANDROID_KEYSTORE_PASSWORD",
      "ANDROID_KEY_PASSWORD",
    ],
  },
  {
    name: ["Playアップロード鍵", "Play upload key", "Play 上传密钥"],
    keys: [
      "PLAY_UPLOAD_KEYSTORE",
      "PLAY_UPLOAD_KEY_ALIAS",
      "PLAY_UPLOAD_KEYSTORE_PASSWORD",
      "PLAY_UPLOAD_KEY_PASSWORD",
    ],
  },
  {
    name: [
      "Playの証明書指紋",
      "Play certificate fingerprints",
      "Play 证书指纹",
    ],
    keys: ["PLAY_APP_SIGNING_SHA256", "PLAY_UPLOAD_SHA256"],
  },
  {
    name: ["Play API認証", "Play API authentication", "Play API 认证"],
    keys: ["GOOGLE_CLOUD_PROJECT", "PLAY_SERVICE_ACCOUNT"],
  },
  {
    name: ["Playリリース設定", "Play release settings", "Play 发布设置"],
    keys: ["PLAY_TRACK", "ANDROID_VERSION_CODE"],
  },
  {
    name: ["Windows署名設定", "Windows signing settings", "Windows 签名设置"],
    keys: [
      "WINDOWS_PUBLISHER",
      "WINDOWS_SIGNING_IDENTITY",
      "WINDOWS_SIGNING_PROVIDER",
    ],
  },
  {
    name: ["PFX証明書", "PFX certificate", "PFX 证书"],
    keys: ["WINDOWS_PFX", "WINDOWS_PFX_PASSWORD"],
  },
  {
    name: ["Azureクラウド署名", "Azure cloud signing", "Azure 云签名"],
    keys: ["AZURE_TENANT_ID", "AZURE_CLIENT_ID", "AZURE_SIGNING_ENDPOINT"],
  },
  {
    name: [
      "Microsoft Storeの製品識別",
      "Microsoft Store product identity",
      "Microsoft Store 产品标识",
    ],
    keys: ["MS_STORE_PRODUCT_ID", "MS_PACKAGE_IDENTITY"],
  },
  {
    name: ["GPG署名鍵", "GPG signing key", "GPG 签名密钥"],
    keys: ["GPG_KEY_ID", "GPG_PRIVATE_KEY"],
  },
  {
    name: ["更新署名鍵", "Updater signing keys", "更新签名密钥"],
    keys: [
      "TAURI_SIGNING_PRIVATE_KEY",
      "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
      "TAURI_UPDATER_PUBLIC_KEY",
    ],
  },
  {
    name: ["更新配信先", "Update delivery", "更新分发"],
    keys: ["TAURI_UPDATER_ENDPOINT", "TAURI_UPDATER_CHANNEL"],
  },
  {
    name: ["更新マニフェスト", "Update manifest", "更新清单"],
    keys: [
      "TAURI_UPDATER_VERSION",
      "TAURI_UPDATER_PLATFORM",
      "TAURI_UPDATER_ARTIFACT",
      "TAURI_UPDATER_SIGNATURE",
      "TAURI_UPDATE_MANIFEST",
    ],
  },
];
const byKey = new Map(
  sets.flatMap((set) => set.keys.map((key) => [key, set] as const)),
);
export function credentialSet(def: Def) {
  return def.guide === "custom" ? undefined : byKey.get(def.key);
}
export function sheetGroups(rows: Row[], t: T) {
  const providers = new Map<string, Map<CredentialSet | undefined, Row[]>>();
  for (const row of rows) {
    const provider = row.def.group;
    const sets =
      providers.get(provider) ?? new Map<CredentialSet | undefined, Row[]>();
    providers.set(provider, sets);
    const set = credentialSet(row.def);
    const members = sets.get(set);
    if (members) members.push(row);
    else sets.set(set, [row]);
  }
  return Array.from(providers).flatMap(([provider, sets]) =>
    Array.from(sets, ([set, members]) => ({
      provider,
      name: set && t(...set.name),
      members,
      id: JSON.stringify([provider, set?.keys[0]]),
    })),
  );
}
