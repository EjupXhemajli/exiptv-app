import { useState, useEffect } from "react";
import { backend } from "../lib/backend";

/**
 * Poster mit robustem Laden:
 * 1. Versucht das Bild direkt anzuzeigen (schnell, für normale URLs).
 * 2. Schlägt das fehl (viele IPTV-Poster brauchen einen User-Agent oder
 *    laden nicht direkt), wird es über das Backend geladen und lokal
 *    gecached – das umgeht die Ladeprobleme.
 * 3. Klappt auch das nicht, erscheint ein dezenter Platzhalter mit dem
 *    Titel-Anfang statt eines kaputten Bild-Icons.
 */
export default function Poster({ src, alt, ratio = "2 / 3" }: { src: string | null; alt: string; ratio?: string }) {
  const [current, setCurrent] = useState<string | null>(src);
  const [triedCache, setTriedCache] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setCurrent(src);
    setTriedCache(false);
    setFailed(false);
  }, [src]);

  // Wenn das direkte Laden fehlschlägt: über das Backend cachen.
  const onError = async () => {
    if (!triedCache && src) {
      setTriedCache(true);
      try {
        const cached = await backend.cacheImage(src);
        setCurrent(cached);
        return;
      } catch {
        // Backend-Cache ebenfalls fehlgeschlagen.
      }
    }
    setFailed(true);
  };

  const show = current && !failed;
  return (
    <div className="poster" style={{ aspectRatio: ratio }}>
      {show ? (
        <img src={current!} alt={alt} loading="lazy" onError={() => void onError()} />
      ) : (
        <div className="poster-fallback" aria-hidden="true">
          <span>{alt.slice(0, 2).toUpperCase()}</span>
        </div>
      )}
    </div>
  );
}
