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
export type Project = { id: number; name: string; targets: string[] };
export type Audit = { at: string; action: string; env: string; key: string };
type Grants = Record<
  "secrets" | "variables" | "environments",
  "none" | "read" | "write"
>;
export type AppConnection = {
  app: string;
  installation: string;
  account: string;
  limits: Grants;
  repos: { id: string; name: string; grants: Grants }[];
};
export type Sheet = {
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
  const today = new Date().toISOString().slice(0, 10);
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
};
