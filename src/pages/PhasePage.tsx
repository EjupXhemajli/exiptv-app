import { useTranslation } from "react-i18next";
import EmptyState from "../components/EmptyState";

/**
 * Ehrliche Zwischenseite für Bereiche kommender Entwicklungsphasen:
 * zeigt den fachlichen Leerzustand plus den Phasenhinweis —
 * keine Fake-Funktionalität.
 */
export default function PhasePage({
  title,
  phase,
  emptyKey,
}: {
  title: string;
  phase: number;
  emptyKey: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <h1>{title}</h1>
      <EmptyState title={title} text={t(emptyKey)} />
      <p className="faint">{t("common.comingPhase", { phase })}</p>
    </>
  );
}
