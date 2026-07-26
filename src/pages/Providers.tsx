import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import EmptyState from "../components/EmptyState";
import type { ImportProgress, ImportReport, Provider, ProviderKind } from "../lib/types";

interface FormState {
  name: string;
  kind: ProviderKind;
  source: string;
  username: string;
  password: string;
}

const EMPTY_FORM: FormState = { name: "", kind: "m3u_url", source: "", username: "", password: "" };

export default function Providers() {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<Provider[] | null>(null);
  const [counts, setCounts] = useState<Record<number, number>>({});
  const [movieCounts, setMovieCounts] = useState<Record<number, number>>({});
  const [seriesCounts, setSeriesCounts] = useState<Record<number, number>>({});
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [formError, setFormError] = useState<string | null>(null);
  const [progress, setProgress] = useState<Record<number, ImportProgress>>({});
  const [reports, setReports] = useState<Record<number, ImportReport>>({});
  const [errors, setErrors] = useState<Record<number, string>>({});
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState("");

  const reload = useCallback(async () => {
    const ps = await backend.listProviders();
    setProviders(ps);
    // Kanal-, Film- und Serienzahlen laden (parallel, je Provider einzeln).
    const c: Record<number, number> = {};
    const m: Record<number, number> = {};
    const s: Record<number, number> = {};
    await Promise.all(ps.map(async (p) => {
      c[p.id] = await backend.countChannels(p.id).catch(() => 0);
      m[p.id] = await backend.countMovies(p.id).catch(() => 0);
      s[p.id] = await backend.countSeries(p.id).catch(() => 0);
    }));
    setCounts(c);
    setMovieCounts(m);
    setSeriesCounts(s);
  }, []);

  useEffect(() => { void reload(); }, [reload]);

  useEffect(() => {
    let un: (() => void) | undefined;
    void backend.onImportProgress((raw) => {
      const p = raw as ImportProgress;
      setProgress((prev) => ({ ...prev, [p.provider_id]: p }));
    }).then((u) => { un = u; });
    return () => un?.();
  }, []);

  const submit = async () => {
    setFormError(null);
    if (!form.name.trim()) { setFormError(t("providers.validationName")); return; }
    if (!form.source.trim()) { setFormError(t("providers.validationSource")); return; }
    const id = await backend.addProvider({
      name: form.name.trim(),
      kind: form.kind,
      source: form.source.trim(),
      username: form.username.trim() || undefined,
      password: form.password || undefined,
    });
    setShowForm(false);
    setForm(EMPTY_FORM);
    await reload();
    await runImport(id, form.kind, form.source.trim());
  };

  const runImport = async (id: number, kind: ProviderKind, source: string) => {
    setErrors((e) => { const n = { ...e }; delete n[id]; return n; });
    try {
      const report =
        kind === "xtream"
          ? await backend.importXtream(id)
          : kind === "m3u_file"
          ? await backend.importM3uFromFile(id, source)
          : await backend.importM3uFromUrl(id, source);
      setReports((r) => ({ ...r, [id]: report }));
      setCounts((c) => ({ ...c, [id]: report.channels_parsed }));
      await reload();
    } catch (err) {
      setErrors((e) => ({ ...e, [id]: String(err) }));
    } finally {
      setProgress((p) => { const n = { ...p }; delete n[id]; return n; });
    }
  };

  const remove = async (id: number) => {
    if (!window.confirm(t("providers.deleteConfirm"))) return;
    await backend.deleteProvider(id);
    await reload();
  };

  const toggleEnabled = async (p: Provider) => {
    await backend.setProviderEnabled(p.id, !p.enabled);
    await reload();
  };

  const startEdit = (p: Provider) => { setEditingId(p.id); setEditName(p.name); };
  const saveEdit = async () => {
    if (editingId == null) return;
    if (editName.trim()) {
      await backend.renameProvider(editingId, editName.trim());
      await reload();
    }
    setEditingId(null);
  };

  const pickFile = async () => {
    const path = await backend.pickM3uFile();
    if (path) setForm((f) => ({ ...f, source: path }));
  };

  return (
    <>
      <header className="row" style={{ justifyContent: "space-between" }}>
        <h1>{t("providers.title")}</h1>
        <button className="primary" onClick={() => setShowForm((s) => !s)}>
          {showForm ? t("providers.cancel") : t("providers.add")}
        </button>
      </header>

      {showForm && (
        <section className="card" style={{ display: "grid", gap: 12, maxWidth: 640 }}>
          <label>
            <span className="dim">{t("providers.name")}</span>
            <input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
          </label>
          <label>
            <span className="dim">{t("providers.type")}</span>
            <select
              value={form.kind}
              onChange={(e) => setForm({ ...form, kind: e.target.value as ProviderKind, source: "" })}
            >
              <option value="m3u_url">{t("providers.typeM3uUrl")}</option>
              <option value="m3u_file">{t("providers.typeM3uFile")}</option>
              <option value="xtream">{t("providers.typeXtream")}</option>
            </select>
          </label>
          <label>
            <span className="dim">{t("providers.source")}</span>
            <div className="row">
              <input
                className="grow"
                value={form.source}
                onChange={(e) => setForm({ ...form, source: e.target.value })}
                placeholder={
                  form.kind === "m3u_url" ? t("providers.sourceUrlPlaceholder")
                  : form.kind === "m3u_file" ? t("providers.sourceFilePlaceholder")
                  : t("providers.serverPlaceholder")
                }
              />
              {form.kind === "m3u_file" && (
                <button onClick={() => void pickFile()}>{t("providers.chooseFile")}</button>
              )}
            </div>
          </label>
          {form.kind === "xtream" && (
            <>
              <label>
                <span className="dim">{t("providers.username")}</span>
                <input value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })} autoComplete="off" />
              </label>
              <label>
                <span className="dim">{t("providers.password")}</span>
                <input type="password" value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })} autoComplete="new-password" />
                <p className="faint">{t("providers.passwordHint")}</p>
              </label>
            </>
          )}
          {formError && <p role="alert" style={{ color: "var(--danger)" }}>{formError}</p>}
          <div className="row">
            <button className="primary" onClick={() => void submit()}>{t("providers.save")}</button>
            <button onClick={() => { setShowForm(false); setFormError(null); }}>{t("providers.cancel")}</button>
          </div>
        </section>
      )}

      {providers === null && <div className="skeleton" style={{ height: 96 }} />}

      {providers !== null && providers.length === 0 && !showForm && (
        <EmptyState
          title={t("providers.emptyTitle")}
          text={t("providers.emptyText")}
          action={<button className="primary" onClick={() => setShowForm(true)}>{t("providers.add")}</button>}
        />
      )}

      {providers?.map((p) => {
        const prog = progress[p.id];
        const rep = reports[p.id];
        const err = errors[p.id];
        const count = counts[p.id];
        return (
          <section key={p.id} className={`card provider-card ${p.enabled ? "" : "disabled"}`} style={{ display: "grid", gap: 8 }}>
            <div className="row" style={{ justifyContent: "space-between", gap: 12 }}>
              <div className="grow" style={{ minWidth: 0 }}>
                {editingId === p.id ? (
                  <div className="row" style={{ gap: 8 }}>
                    <input
                      className="grow"
                      value={editName}
                      autoFocus
                      onChange={(e) => setEditName(e.target.value)}
                      onKeyDown={(e) => { if (e.key === "Enter") void saveEdit(); if (e.key === "Escape") setEditingId(null); }}
                    />
                    <button className="primary" onClick={() => void saveEdit()}>{t("providers.saveName")}</button>
                    <button onClick={() => setEditingId(null)}>{t("providers.cancel")}</button>
                  </div>
                ) : (
                  <>
                    <h2 style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {p.name}
                      {!p.enabled && <span className="faint" style={{ fontSize: 13, marginLeft: 8 }}>· {t("providers.disabled")}</span>}
                    </h2>
                    <p className="faint">
                      {kindLabel(p.kind, t)}
                      {count != null && <> · {t("providers.channelCount", { count })}</>}
                      {(movieCounts[p.id] ?? 0) > 0 && <> · {t("providers.importMovies", { count: movieCounts[p.id] })}</>}
                      {(seriesCounts[p.id] ?? 0) > 0 && <> · {t("providers.importSeries", { count: seriesCounts[p.id] })}</>}
                      {" · "}
                      {t("providers.lastRefresh", {
                        time: p.last_refresh_at
                          ? new Date(p.last_refresh_at * 1000).toLocaleString()
                          : t("providers.never"),
                      })}
                    </p>
                  </>
                )}
              </div>
              {editingId !== p.id && (
                <div className="row" style={{ gap: 8, flexShrink: 0 }}>
                  {/* An/Aus-Schalter */}
                  <button
                    className={`switch ${p.enabled ? "on" : ""}`}
                    role="switch"
                    aria-checked={p.enabled}
                    aria-label={t("providers.toggleEnabled")}
                    title={p.enabled ? t("providers.disable") : t("providers.enable")}
                    onClick={() => void toggleEnabled(p)}
                    disabled={!!prog}
                  >
                    <span className="knob" />
                  </button>
                </div>
              )}
            </div>

            {editingId !== p.id && (
              <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
                <button onClick={() => startEdit(p)} disabled={!!prog}>{t("providers.rename")}</button>
                <button onClick={() => void runImport(p.id, p.kind, p.source)} disabled={!!prog}>
                  {t("providers.refresh")}
                </button>
                <button className="danger" onClick={() => void remove(p.id)} disabled={!!prog}>
                  {t("providers.delete")}
                </button>
              </div>
            )}

            {prog && (
              <p className="dim" role="status">
                {prog.stage === "laden" && t("providers.importStageLaden")}
                {prog.stage === "verarbeiten" && t("providers.importStageVerarbeiten")}
                {prog.stage === "speichern" && t("providers.importStageSpeichern", { count: prog.channels })}
              </p>
            )}
            {rep && !prog && (
              <>
                <p className="dim">
                  {t("providers.importDone", { count: rep.channels_parsed })}
                  {rep.channels_skipped > 0 && <> · {t("providers.importSkipped", { count: rep.channels_skipped })}</>}
                  {(rep.movies_parsed ?? 0) > 0 && <> · {t("providers.importMovies", { count: rep.movies_parsed })}</>}
                  {(rep.series_parsed ?? 0) > 0 && <> · {t("providers.importSeries", { count: rep.series_parsed })}</>}
                  {rep.encoding && <span className="faint"> · {rep.encoding}</span>}
                </p>
                {rep.warnings && rep.warnings.length > 0 && (
                  <p className="faint" style={{ color: "var(--warning)" }}>{rep.warnings[0]}</p>
                )}
              </>
            )}
            {err && <p role="alert" style={{ color: "var(--danger)" }}>{err}</p>}
          </section>
        );
      })}
    </>
  );
}

function kindLabel(kind: ProviderKind, t: (k: string) => string): string {
  switch (kind) {
    case "m3u_url": return t("providers.typeM3uUrl");
    case "m3u_file": return t("providers.typeM3uFile");
    case "xtream": return t("providers.typeXtream");
    default: return kind;
  }
}
