import { useCallback, useEffect, useRef, useState } from "react";
import { backend } from "../lib/backend";

/**
 * Ziehbare Spaltenbreite. Persistiert den Wert unter `settingKey`
 * (App-Setting), sodass die vom Nutzer gewählte Breite erhalten bleibt.
 */
export function useResizable(settingKey: string, initial: number, min: number, max: number) {
  const [width, setWidth] = useState(initial);
  const dragging = useRef(false);
  const startX = useRef(0);
  const startW = useRef(0);

  useEffect(() => {
    backend.getSetting(settingKey).then((v) => {
      const n = v ? parseInt(v, 10) : NaN;
      if (!Number.isNaN(n)) setWidth(Math.min(max, Math.max(min, n)));
    });
  }, [settingKey, min, max]);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    dragging.current = true;
    startX.current = e.clientX;
    startW.current = width;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    e.preventDefault();
  }, [width]);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      const delta = e.clientX - startX.current;
      setWidth(Math.min(max, Math.max(min, startW.current + delta)));
    };
    const onUp = () => {
      if (!dragging.current) return;
      dragging.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      void backend.setSetting(settingKey, String(Math.round(widthRef.current)));
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [settingKey, min, max]);

  // aktuelle Breite für den Mouseup-Handler ohne Re-Bind.
  const widthRef = useRef(width);
  widthRef.current = width;

  return { width, onMouseDown };
}
