import { Navigate, Route, Routes, useNavigate, useLocation } from "react-router-dom";
import { useEffect, useRef } from "react";
import Sidebar from "./components/Sidebar";
import Home from "./pages/Home";
import LiveTV from "./pages/LiveTV";
import Providers from "./pages/Providers";
import Search from "./pages/Search";
import Settings from "./pages/Settings";
import PhasePage from "./pages/PhasePage";
import Movies from "./pages/Movies";
import SeriesPage from "./pages/Series";
import Favorites from "./pages/Favorites";
import History from "./pages/History";
import { useTranslation } from "react-i18next";
import { useSettings } from "./state/settingsStore";

const START_ROUTES: Record<string, string> = {
  home: "/", livetv: "/live", movies: "/movies", series: "/series", favorites: "/favorites",
};

export default function App() {
  const { t } = useTranslation();
  const loadSettings = useSettings((s) => s.load);
  const loaded = useSettings((s) => s.loaded);
  const startView = useSettings((s) => s.settings.startView);
  const navigate = useNavigate();
  const location = useLocation();
  const didRedirect = useRef(false);

  useEffect(() => { void loadSettings(); }, [loadSettings]);

  // Einmalig beim Start zur gewünschten Startansicht springen.
  useEffect(() => {
    if (loaded && !didRedirect.current && location.pathname === "/") {
      didRedirect.current = true;
      const target = START_ROUTES[startView] ?? "/";
      if (target !== "/") navigate(target, { replace: true });
    }
  }, [loaded, startView, location.pathname, navigate]);

  return (
    <div className="app-shell">
      <Sidebar />
      <main className="app-main">
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/live" element={<LiveTV />} />
          <Route path="/guide" element={<PhasePage title={t("nav.guide")} phase={6} emptyKey="empty.guide" />} />
          <Route path="/movies" element={<Movies />} />
          <Route path="/series" element={<SeriesPage />} />
          <Route path="/favorites" element={<Favorites />} />
          <Route path="/recordings" element={<PhasePage title={t("nav.recordings")} phase={9} emptyKey="empty.recordings" />} />
          <Route path="/history" element={<History />} />
          <Route path="/search" element={<Search />} />
          <Route path="/providers" element={<Providers />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </main>
    </div>
  );
}
