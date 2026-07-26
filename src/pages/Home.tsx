import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import EmptyState from "../components/EmptyState";
import type { Provider } from "../lib/types";

function greetingKey(): string {
  const h = new Date().getHours();
  if (h < 11) return "home.greetingMorning";
  if (h < 18) return "home.greetingDay";
  return "home.greetingEvening";
}

export default function Home() {
  const { t } = useTranslation();
  const nav = useNavigate();
  const [providers, setProviders] = useState<Provider[] | null>(null);

  useEffect(() => {
    backend.listProviders().then(setProviders).catch(() => setProviders([]));
  }, []);

  return (
    <>
      <header>
        <h1>{t(greetingKey())}</h1>
        <p className="dim">{t("home.subtitle")}</p>
      </header>

      {providers === null && (
        <div className="row" style={{ gap: "var(--gap-m)" }}>
          <div className="skeleton" style={{ height: 120, flex: 1 }} />
          <div className="skeleton" style={{ height: 120, flex: 1 }} />
          <div className="skeleton" style={{ height: 120, flex: 1 }} />
        </div>
      )}

      {providers !== null && providers.length === 0 && (
        <EmptyState
          title={t("home.getStartedTitle")}
          text={t("home.getStartedText")}
          action={
            <button className="primary" onClick={() => nav("/providers")}>
              {t("home.getStartedAction")}
            </button>
          }
        />
      )}

      {providers !== null && providers.length > 0 && (
        <section className="card">
          <h2>{t("home.recentChannels")}</h2>
          <p className="dim" style={{ marginTop: 8 }}>{t("empty.history")}</p>
        </section>
      )}
    </>
  );
}
