import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./overlay.css";

type R = { x: number; y: number; w: number; h: number };
type Hint = { id: number; typed: string; result: string; hint: boolean; caret: R | null; word: R | null; mouse: R | null };

export function Overlay() {
  const [h, setH] = useState<Hint | null>(null);

  useEffect(() => {
    let timer = 0;
    const off = listen<Hint>("hint", (e) => {
      setH(e.payload);
      clearTimeout(timer);
      timer = window.setTimeout(() => setH(null), 1650);
    });
    return () => { off.then((f) => f()); clearTimeout(timer); };
  }, []);

  if (!h) return null;
  const k = window.devicePixelRatio || 1;
  const anchor = h.word ?? h.caret;
  const badge = h.hint && (anchor || h.mouse) ? (() => {
    if (h.word) return { left: h.word.x / k - 8, top: h.word.y / k };
    if (h.caret) return { left: h.caret.x / k - 17, top: h.caret.y / k };
    return { left: h.mouse!.x / k - 17, top: h.mouse!.y / k - 4 };
  })() : null;

  return (
    <div className="ov" key={h.id}>
      {badge && (
        <span className="ov-badge" style={{ left: badge.left, top: badge.top }}>
          <s>{h.typed}</s>
        </span>
      )}
      {h.hint && h.word && (
        <span className="ov-line" style={{ left: h.word.x / k, top: (h.word.y + h.word.h) / k - 2, width: h.word.w / k }} />
      )}
      {h.mouse && (
        <span className="ov-star" style={{ left: h.mouse.x / k + 13, top: h.mouse.y / k - 13 }}>
          <svg viewBox="0 0 16 16"><path d="M8 0.5 9.6 6.4 15.5 8 9.6 9.6 8 15.5 6.4 9.6 0.5 8 6.4 6.4Z" fill="currentColor" /></svg>
        </span>
      )}
    </div>
  );
}
