import { Hexagon, KeyRound, Globe, type LucideIcon } from "lucide-react";
import { type Lang } from "./guide";
export const floating = new URLSearchParams(location.search).has("panel");
export function Mark() {
  return (
    <span className="mark">
      <Hexagon size={33} />
      <KeyRound size={13} />
    </span>
  );
}
export function Btn({
  icon: I,
  title,
  click,
  disabled = false,
}: {
  icon: LucideIcon;
  title: string;
  click: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className="icon"
      aria-label={title}
      title={title}
      onClick={click}
      disabled={disabled}
    >
      <I size={16} />
    </button>
  );
}
export function Language({
  lang,
  change,
}: {
  lang: Lang;
  change: (l: Lang) => void;
}) {
  return (
    <label className="language">
      <Globe size={15} />
      <select
        aria-label="Language"
        value={lang}
        onChange={(e) => change(e.target.value as Lang)}
      >
        <option value="ja">日本語</option>
        <option value="en">English</option>
        <option value="zh">简体中文</option>
      </select>
    </label>
  );
}
