import appMark from "../brand/logo/mark-small.svg";

const PATHS: Record<string, string> = {
  settings: "M20 7h-9M14 17H5M20 17a3 3 0 1 1-6 0 3 3 0 0 1 6 0ZM10 7a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
  x: "M18 6 6 18M6 6l12 12",
  plus: "M12 5v14M5 12h14",
  min: "M5 12h14",
  repl: "M4 7h9M4 12h6M4 17h5M17 5v13M13.5 14.5 17 18l3.5-3.5",
  eyeOff: "M10.7 5.1A10.7 10.7 0 0 1 12 5c7 0 10 7 10 7a13.2 13.2 0 0 1-1.7 2.7M6.6 6.6A13.5 13.5 0 0 0 2 12s3 7 10 7a9.7 9.7 0 0 0 5.4-1.6M2 2l20 20M9.9 9.9a3 3 0 1 0 4.2 4.2",
  app: "M5.5 4h13A2.5 2.5 0 0 1 21 6.5v11a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 17.5v-11A2.5 2.5 0 0 1 5.5 4ZM3 9h18",
  keyboard: "M4 5h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2ZM6 9h.01M10 9h.01M14 9h.01M18 9h.01M8 13h.01M12 13h.01M16 13h.01M7 16h10",
  info: "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM12 11v5.5M12 7.6v.01",
  book: "M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20",
  search: "M17.5 11a6.5 6.5 0 1 1-13 0 6.5 6.5 0 0 1 13 0ZM16 16l4 4",
  swap: "M4 8h15l-3.5-3.5M20 16H5l3.5 3.5",
  chev: "m6 9 6 6 6-6",
  check: "M5 12.5 10 17.5 19 7",
  restart: "M20 12a8 8 0 1 1-2.3-5.7M20 4v4.5h-4.5",
};

export function Icon({ name, className = "" }: { name: keyof typeof PATHS | string; className?: string }) {
  return (
    <svg className={`gb-ic ${className}`} viewBox="0 0 24 24" aria-hidden="true">
      <path d={PATHS[name]} fill="none" stroke="currentColor" strokeWidth={name === "close" ? 1.25 : 1.5} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function Mark({ look = "on", ru = true, className = "gb-mk" }: { look?: "on" | "mute" | "off"; ru?: boolean; className?: string }) {
  const plate =
    look === "off" ? <rect x="1.1" y="1.6" width="13.8" height="12.8" rx="3" fill="none" stroke="var(--dim)" strokeWidth="1.2" />
    : <rect x="0.5" y="1" width="15" height="14" rx="3.5" fill={look === "on" ? "var(--gb)" : "var(--hover-strong)"} />;
  const ink = look === "on" ? "var(--gb-ink)" : look === "mute" ? "var(--fg)" : "var(--dim)";
  return (
    <svg className={className} viewBox="0 0 16 16" aria-hidden="true">
      {plate}
      <text x="8" y="11.1" textAnchor="middle" fontFamily="Segoe UI, sans-serif" fontSize="8.2" letterSpacing="-0.2" fontWeight="700" fill={ink}>{ru ? "RU" : "EN"}</text>
    </svg>
  );
}

export function AppMark({ className }: { className: string }) {
  return <img className={className} src={appMark} alt="" />;
}

export function Kbd({ keys }: { keys: string }) {
  return (
    <span className="t-kbds">
      {keys.split("+").map((k) => <span key={k} className="t-kbd">{k}</span>)}
    </span>
  );
}

export function Toggle({ on, label, onChange, dim }: { on: boolean; label: string; onChange: (v: boolean) => void; dim?: boolean }) {
  return <button type="button" className={`t-tg${dim ? " gb-dis" : ""}`} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />;
}
