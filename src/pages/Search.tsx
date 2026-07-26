import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import type { Channel } from "../lib/types";

export default function Search() {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Channel[]>([]);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounce = useRef<number>();

  useEffect(() => { inputRef.current?.focus(); }, []);

  // Suche während der Eingabe, entprellt.
  useEffect(() => {
    window.clearTimeout(debounce.current);
    if (query.trim().length < 2) { setResults([]); return; }
    setBusy(true);
    debounce.current = window.setTimeout(() => {
      backend.searchChannels(query)
        .then(setResults)
        .finally(() => setBusy(false));
    }, 220);
    return () => window.clearTimeout(debounce.current);
  }, [query]);

  return (
    <>
      <h1>{t("nav.search")}</h1>
      <input
        ref={inputRef}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder={t("search.placeholder")}
        aria-label={t("nav.search")}
      />
      {busy && <div className="skeleton" style={{ height: 44 }} />}
      {!busy && query.trim().length >= 2 && results.length === 0 && (
        <p className="dim">{t("search.noResults", { query })}</p>
      )}
      {results.length > 0 && (
        <div className="card" style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          {results.map((c) => (
            <div key={`${c.provider_id}-${c.id}`} className="row" style={{ padding: "8px 6px" }}>
              <span className="faint" style={{ width: 44, textAlign: "right" }}>
                {c.channel_number ?? "—"}
              </span>
              <span className="grow">{c.name}</span>
              {c.is_radio && <span className="faint">Radio</span>}
            </div>
          ))}
        </div>
      )}
      <p className="faint">{t("search.hint")}</p>
    </>
  );
}
