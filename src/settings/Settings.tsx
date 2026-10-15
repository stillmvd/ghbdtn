import { useCallback, useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { checkNow, installNow, UpdateBadge, UpdateToast, useShip } from "@stillmvd/tauri-ship";
import { api, errorsOf, type About, type Config, type ConfigError, type Proc } from "../api";
import { Icon, Kbd, Mark, Toggle } from "../Icon";
import "./settings.css";

type Sec = "main" | "repl" | "never" | "proc" | "keys" | "about";
const NAV: [Sec, string, string][] = [
  ["main", "Основное", "settings"],
  ["repl", "Замены", "repl"],
  ["never", "Не трогать", "eyeOff"],
  ["proc", "Исключения", "app"],
  ["keys", "Клавиши", "keyboard"],
  ["about", "О программе", "info"],
];

type Ctx = { cfg: Config; save: (next: Config) => Promise<boolean>; errors: ConfigError[]; setCfg: (c: Config) => void };

function Row({ title, sub, children }: { title: string; sub?: string; children: ReactNode }) {
  return (
    <div className="t-setrow">
      <span className="tx"><b>{title}</b>{sub && <span>{sub}</span>}</span>
      {children}
    </div>
  );
}

function Group({ label, extra, note, children }: { label: string; extra?: ReactNode; note?: string; children: ReactNode }) {
  return (
    <div className="st-grp">
      <div className="st-grp-h"><span className="mono-label st-lbl">{label}</span>{extra}</div>
      {note && <p className="st-note">{note}</p>}
      {children}
    </div>
  );
}

const Err = ({ list, prefix }: { list: ConfigError[]; prefix: string }) => {
  const e = list.find((x) => x.field.startsWith(prefix));
  return e ? <p className="t-field-msg err">{e.message}</p> : null;
};

function Main({ cfg, save }: Ctx) {
  const f = (k: keyof Config["features"]) => (v: boolean) => save({ ...cfg, features: { ...cfg.features, [k]: v } });
  const g = (k: keyof Config["signals"]) => (v: boolean) => save({ ...cfg, signals: { ...cfg.signals, [k]: v } });
  return (
    <>
      <Row title="Включено"><Kbd keys={cfg.hotkeys.toggle} /><Toggle on={cfg.enabled} label="Включено" onChange={(v) => save({ ...cfg, enabled: v })} /></Row>
      <div className="st-cols">
        <div className="st-stack">
          <Group label="Исправлять">
            <Row title="Раскладку" sub="ghbdtn → привет"><Toggle on={cfg.features.layout} label="Раскладку" onChange={f("layout")} /></Row>
            <Row title="Опечатки" sub="ввожа → ввода"><Toggle on={cfg.features.typos} label="Опечатки" onChange={f("typos")} /></Row>
            <Row title="Замены" sub="чето → чё-то"><Toggle on={cfg.features.replace} label="Замены" onChange={f("replace")} /></Row>
          </Group>
          <Group label="Windows">
            <Row title="Запускать с Windows"><Toggle on={cfg.autostart} label="Запускать с Windows" onChange={(v) => save({ ...cfg, autostart: v })} /></Row>
          </Group>
          <Group label="Журнал">
            <Row title="Вести журнал" sub="Исправления со словами — только на этом компьютере, 30 дней"><Toggle on={cfg.journal} label="Вести журнал" onChange={(v) => save({ ...cfg, journal: v })} /></Row>
          </Group>
        </div>
        <Group label="Показывать">
          <Row title="Подсказку у курсора"><Toggle on={cfg.signals.hint} label="Подсказку у курсора" onChange={g("hint")} /></Row>
          <Row title="Мигание значка"><Toggle on={cfg.signals.blink} label="Мигание значка" onChange={g("blink")} /></Row>
          <Row title="Анимацию курсора"><Toggle on={cfg.signals.cursor} label="Анимацию курсора" onChange={g("cursor")} /></Row>
          <Row title="Звук"><Toggle on={cfg.signals.sound} label="Звук" onChange={g("sound")} /></Row>
        </Group>
      </div>
    </>
  );
}

function Repl({ cfg, save, errors }: Ctx) {
  const [k, setK] = useState("");
  const [v, setV] = useState("");
  const add = async () => {
    const key = k.trim().toLowerCase(), val = v.trim();
    if (!key || !val) return;
    if (await save({ ...cfg, replacements: { ...cfg.replacements, [key]: val } })) { setK(""); setV(""); }
  };
  const del = (key: string) => {
    const next = { ...cfg.replacements };
    delete next[key];
    save({ ...cfg, replacements: next });
  };
  const bad = errors.some((e) => e.field.startsWith("replacements"));
  return (
    <>
      <div className="st-add-wrap">
        <form className="st-add" onSubmit={(e) => { e.preventDefault(); add(); }}>
          <label className={`t-field${bad ? " err" : ""}`}><input id="repl-key" value={k} onChange={(e) => setK(e.target.value)} placeholder="Набираешь" aria-label="Набираешь" /></label>
          <span className="gb-ar">→</span>
          <label className={`t-field${bad ? " err" : ""}`}><input id="repl-val" value={v} onChange={(e) => setV(e.target.value)} placeholder="Получаешь" aria-label="Получаешь" /></label>
          <button type="submit" className="t-btn p" disabled={!k.trim() || !v.trim()}><Icon name="plus" />Добавить</button>
        </form>
        <Err list={errors} prefix="replacements" />
      </div>
      <div className="st-list">
        {Object.entries(cfg.replacements).map(([a, b]) => (
          <span key={a}><b className="k">{a}</b><span className="gb-ar">→</span><span className="v">{b}</span>
            <button type="button" className="gb-x" aria-label={`Удалить ${a}`} onClick={() => del(a)}><Icon name="x" /></button></span>
        ))}
      </div>
    </>
  );
}

function Never({ cfg, setCfg }: Ctx) {
  const run = (p: Promise<Config>) => p.then(setCfg).catch(() => {});
  return (
    <Group label="Слова" extra={<><span className="n">{cfg.never.length}</span>{cfg.never.length > 0 && <button type="button" className="t-btn s32" onClick={() => run(api.clearNever())}>Очистить</button>}</>}>
      {cfg.never.length === 0 ? (
        <div className="t-empty"><span className="circ"><Icon name="eyeOff" /></span><b>Слов нет</b><span>Слово попадёт сюда, если дважды отменить его исправление Ctrl+Backspace.</span></div>
      ) : (
        <div className="st-list">
          {cfg.never.map((n) => (
            <span key={n.word}><b className="k v">{n.word}</b><span className="src">{n.source === "undo" ? "после двух отмен" : "вручную"}</span>
              <button type="button" className="gb-x" aria-label={`Удалить ${n.word}`} onClick={() => run(api.removeNever(n.word))}><Icon name="x" /></button></span>
          ))}
        </div>
      )}
    </Group>
  );
}

function ProcGroup({ cfg, save, kind, label, note }: Ctx & { kind: "off" | "code"; label: string; note?: string }) {
  const [adding, setAdding] = useState(false);
  const [val, setVal] = useState("");
  const [procs, setProcs] = useState<Proc[]>([]);
  const list = cfg.processes[kind];
  const set = (next: string[]) => save({ ...cfg, processes: { ...cfg.processes, [kind]: next } });
  const open = () => { setAdding(true); api.listProcesses().then(setProcs).catch(() => {}); };
  const add = async () => {
    const exe = val.trim().toLowerCase();
    if (!exe) return;
    if (await set([...list, exe.endsWith(".exe") ? exe : `${exe}.exe`])) { setVal(""); setAdding(false); }
  };
  return (
    <Group label={label} note={note} extra={<><span className="n">{list.length}</span><button type="button" className="t-btn s32" onClick={open}><Icon name="plus" />Добавить</button></>}>
      {adding && (
        <form className="st-add" onSubmit={(e) => { e.preventDefault(); add(); }}>
          <label className="t-field"><input id={`proc-${kind}`} autoFocus list={`procs-${kind}`} value={val} onChange={(e) => setVal(e.target.value)} placeholder="Процесс, например telegram.exe" aria-label="Процесс"
            onKeyDown={(e) => { if (e.key === "Escape") setAdding(false); }} /></label>
          <datalist id={`procs-${kind}`}>{procs.filter((p) => !list.includes(p.exe)).map((p) => <option key={p.exe} value={p.exe}>{p.title}</option>)}</datalist>
          <button type="submit" className="t-btn p" disabled={!val.trim()}>Добавить</button>
        </form>
      )}
      <div className="st-list">
        {list.map((p) => (
          <span key={p}><span className="v gb-mono">{p}</span>
            <button type="button" className="gb-x" aria-label={`Удалить ${p}`} onClick={() => set(list.filter((x) => x !== p))}><Icon name="x" /></button></span>
        ))}
      </div>
    </Group>
  );
}

function Procs(ctx: Ctx) {
  return (
    <div className="st-cols">
      <ProcGroup {...ctx} kind="off" label="Выключено" />
      <ProcGroup {...ctx} kind="code" label="Режим кода" note="Флаги, пути и код не трогаю" />
    </div>
  );
}

const MODS = new Set(["Control", "Shift", "Alt", "Meta"]);
function keyName(e: KeyboardEvent): string | null {
  if (MODS.has(e.key)) return null;
  let key = e.key;
  if (e.code.startsWith("Key")) key = e.code.slice(3);
  else if (e.code.startsWith("Digit")) key = e.code.slice(5);
  else if (e.code === "Pause" || e.key === "Pause") key = "Pause";
  else if (e.code === "ScrollLock") key = "ScrollLock";
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean);
  return [...mods, key].join("+");
}

function Keys({ cfg, save, errors }: Ctx) {
  const [wait, setWait] = useState<"convert" | "toggle" | null>(null);
  useEffect(() => {
    if (!wait) return;
    const enabled = cfg.enabled;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Escape") { setWait(null); return; }
      const name = keyName(e);
      if (!name) return;
      setWait(null);
      save({ ...cfg, enabled, hotkeys: { ...cfg.hotkeys, [wait]: name } });
    };
    addEventListener("keydown", onKey, true);
    return () => removeEventListener("keydown", onKey, true);
  }, [wait, cfg, save]);
  const change = (k: "convert" | "toggle") => (
    <button type="button" className="t-btn s32" onClick={() => setWait(wait === k ? null : k)}>{wait === k ? "Нажми клавиши…" : "Изменить"}</button>
  );
  return (
    <div className="st-grp">
      <Row title="Перевести слово или выделение"><Kbd keys={cfg.hotkeys.convert} />{change("convert")}</Row>
      <Err list={errors} prefix="hotkeys.convert" />
      <Row title="Выключить и включить"><Kbd keys={cfg.hotkeys.toggle} />{change("toggle")}</Row>
      <Err list={errors} prefix="hotkeys.toggle" />
      <Row title="Отменить исправление" sub="сразу после исправления"><Kbd keys="Ctrl+Backspace" /></Row>
    </div>
  );
}

function ago(ms: number) {
  const m = Math.floor((Date.now() - ms) / 60_000);
  return m < 1 ? "только что" : m < 60 ? `${m} мин назад` : `в ${new Date(ms).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" })}`;
}

function Updates() {
  const { status } = useShip();
  const [checking, setChecking] = useState(false);
  const phase = status?.phase ?? "idle";
  const busy = checking || phase === "checking" || phase === "downloading" || phase === "installing";
  const sub = status?.error
    ? status.error
    : phase === "ready" ? `Готова версия ${status?.available ?? ""}`
    : phase === "downloading" ? "Скачиваю обновление"
    : busy ? "Проверяю"
    : status?.lastCheck ? `Последняя версия · проверено ${ago(status.lastCheck)}` : "";
  const check = async () => { setChecking(true); try { await checkNow(); } finally { setChecking(false); } };
  return (
    <Row title="Обновления" sub={sub}>
      {phase === "ready"
        ? <button type="button" className="t-btn s32 p" onClick={() => installNow()}>Перезапустить</button>
        : <button type="button" className="t-btn s32" disabled={busy} onClick={check}>Проверить</button>}
    </Row>
  );
}

function AboutSec() {
  const [a, setA] = useState<About | null>(null);
  useEffect(() => { api.about().then(setA).catch(() => {}); }, []);
  if (!a) return null;
  return (
    <>
      <Row title={`Ghbdtn ${a.version}`} sub="Набранный текст не пишется на диск и никуда не отправляется"><Mark /></Row>
      <Updates />
      <Group label="Словари">
        {a.licenses.map((l) => <Row key={l.name} title={l.name} sub={`${l.source} · ${l.license}`}><span /></Row>)}
      </Group>
    </>
  );
}

export function Settings() {
  const [sec, setSec] = useState<Sec>("main");
  const [cfg, setCfg] = useState<Config | null>(null);
  const [errors, setErrors] = useState<ConfigError[]>([]);
  const win = getCurrentWindow();

  useEffect(() => {
    api.getConfig().then(setCfg).catch(() => {});
    const off = listen<Config>("config-changed", (e) => setCfg(e.payload));
    return () => { off.then((f) => f()); };
  }, []);

  const save = useCallback(async (next: Config) => {
    try {
      setCfg(await api.setConfig(next));
      setErrors([]);
      return true;
    } catch (e) {
      setErrors(errorsOf(e));
      return false;
    }
  }, []);

  const ctx: Ctx | null = cfg && { cfg, save, errors, setCfg };
  const general = errors.filter((e) => !e.field.startsWith("replacements") && !e.field.startsWith("hotkeys"));
  const Body = { main: Main, repl: Repl, never: Never, proc: Procs, keys: Keys, about: AboutSec }[sec];

  return (
    <div className="st">
      <div className="t-titlebar" data-tauri-drag-region>
        <Mark className="st-mk" />
        <span className="name" data-tauri-drag-region>Ghbdtn</span>
        <span className="sp" data-tauri-drag-region />
        <UpdateBadge />
        <div className="t-win">
          <button type="button" aria-label="Свернуть" onClick={() => win.minimize()}><Icon name="min" /></button>
          <button type="button" className="close" aria-label="Закрыть" onClick={() => win.close()}><Icon name="x" /></button>
        </div>
      </div>
      <div className="st-body">
        <nav className="st-rail" aria-label="Разделы">
          {NAV.map(([k, l, i]) => (
            <button key={k} type="button" className="t-row" aria-current={sec === k ? "page" : undefined} onClick={() => { setSec(k); setErrors([]); }}>
              <Icon name={i} />{l}
            </button>
          ))}
        </nav>
        <main className="st-main">
          <div className="st-sec">
            {general.map((e) => <p key={e.field} className="t-field-msg err">{e.message}</p>)}
            {ctx && <Body {...ctx} />}
          </div>
        </main>
      </div>
      <UpdateToast lang="ru" />
    </div>
  );
}
