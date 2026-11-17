import type { Lang } from "./guide";
export type Scope = { project: number; target: string; env: string };
export type Def = {
  name: string;
  key: string;
  group: string;
  guide: string;
  kind: string;
  targets: string[];
};
export type Row = {
  def: Def;
  present: boolean;
  truncated?: boolean;
  owner?: number | null;
  dependents: number[];
  sources: number[];
  appIdPrefix?: "team" | "custom" | null;
  appleCheck?: {
    kind: string;
    id: string;
    name: string;
    checked: string;
    state: string;
  } | null;
  appPasswordLabel?: string | null;
  fingerprint?: string | null;
  display: string | null;
  expires: string | null;
  synced: string | null;
  binding?: {
    repo: string;
    environment: string;
    automatic: boolean;
    polled: string;
    remote: string;
  } | null;
  evidence: string | null;
};
export type Project = { id: number; name: string; targets: string[]; development_directory?: string };
export type Audit = { at: string; action: string; env: string; key: string };
type Grants = Record<
  | "secrets"
  | "variables"
  | "environments"
  | "actions"
  | "contents"
  | "workflows"
  | "pull_requests",
  "none" | "read" | "write"
>;
export type AppConnection = {
  app: string;
  installation: string;
  account: string;
  limits: Grants;
  repos: { id: string; name: string; grants: Grants }[];
};
export type ExpiryState =
  "invalid" | "expired" | "soon" | "unchecked" | "unknown" | "valid";
export type ExpiryItem = {
  scope: Scope;
  def: Def;
  source: string;
  source_id: number;
  projects: { id: number; name: string }[];
  expires: string;
  days: number | null;
  checked: string | null;
  flags: ExpiryState[];
  state: ExpiryState;
};
export type Sheet = {
  expiryOverview?: { as_of: string; items: ExpiryItem[] };
  appleReport?: {
    id: string;
    checked: string;
    credentialsCurrent: boolean;
    resources: {
      kind: string;
      id: string;
      name: string;
      category: string;
      expires: string;
      state: string;
    }[];
  } | null;
  workflowPreview?: {
    id: string;
    repo: string;
    base: string;
    baseSha: string;
    path: string;
    diff: string;
    branch: string;
    pr: number | null;
    changed: boolean;
  } | null;
  releaseDraft?: {
    version: string;
    platforms: {
      platform: string;
      target: string;
      url: string;
      verified: string;
      keyCurrent?: boolean;
      remote?: { checked: string; status: string; bytes: number | null } | null;
    }[];
  } | null;
  actionReport?: {
    repo: string;
    checked: string;
    runs: {
      id: number;
      name: string;
      branch: string;
      sha: string;
      event: string;
      status: string;
      conclusion: string | null;
      updated: string;
      attempt: number;
    }[];
  } | null;
  githubApp?: AppConnection | null;
  locked: false;
  rows: Row[];
  projects: Project[];
  scope: Scope;
  selected: string;
  floating: boolean;
  deviceEnabled: boolean;
  deviceAvailable: boolean;
  passkeyAvailable: boolean;
  passkeyEnabled: boolean;
  audit: Audit[];
};
export type Snapshot =
  | Sheet
  | {
      locked: true;
      exists: boolean;
      deviceEnabled: boolean;
      passkeyEnabled: boolean;
    };
export type T = (ja: string, en: string, zh: string) => string;
export function status(row: Row) {
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  if (row.appleCheck?.kind === "profiles" && row.appleCheck.state !== "ACTIVE")
    return "invalid";
  return !row.present
    ? "missing"
    : row.expires && row.expires < today
      ? "expired"
      : row.expires &&
          Date.parse(row.expires) - Date.parse(today) <= 30 * 86400000
        ? "soon"
        : "configured";
}

export type FormValues = Record<string, string | string[]>;

export type WorkProps = {
  snap: Sheet;
  row?: Row;
  lang: Lang;
  t: T;
  changeLang: (l: Lang) => void;
  busy: boolean;
  show: (m: string) => void;
  action: (op: string, d?: FormValues) => void;
  lock: () => void;
  revealed: string;
  select: (s: Scope, k: string) => void;
  toggle: () => void;
  openPreferences: () => void;
};
