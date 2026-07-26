import { useEffect, useState } from "react";
import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import {
  IconChevron, IconGear, IconGuide, IconHistory, IconHome, IconLive,
  IconMovie, IconRec, IconSearch, IconSeries, IconServer, IconStar,
} from "./Icons";
import "./sidebar.css";
const logo = "/assets/branding/EXIPTV-LOGO-transparent.png";

const ITEMS = [
  { to: "/", key: "nav.home", icon: <IconHome /> },
  { to: "/live", key: "nav.live", icon: <IconLive /> },
  { to: "/guide", key: "nav.guide", icon: <IconGuide /> },
  { to: "/movies", key: "nav.movies", icon: <IconMovie /> },
  { to: "/series", key: "nav.series", icon: <IconSeries /> },
  { to: "/favorites", key: "nav.favorites", icon: <IconStar /> },
  { to: "/recordings", key: "nav.recordings", icon: <IconRec /> },
  { to: "/history", key: "nav.history", icon: <IconHistory /> },
  { to: "/search", key: "nav.search", icon: <IconSearch /> },
  { to: "/providers", key: "nav.providers", icon: <IconServer /> },
  { to: "/settings", key: "nav.settings", icon: <IconGear /> },
] as const;

export default function Sidebar() {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(false);

  // Zustand über Neustarts hinweg merken (App-Setting).
  useEffect(() => {
    backend.getSetting("ui.sidebar_collapsed").then((v) => setCollapsed(v === "1"));
  }, []);
  const toggle = () => {
    const next = !collapsed;
    setCollapsed(next);
    void backend.setSetting("ui.sidebar_collapsed", next ? "1" : "0");
  };

  return (
    <nav className={`sidebar ${collapsed ? "collapsed" : ""}`} aria-label="Hauptnavigation">
      <div className="sidebar-brand">
        <img src={logo} alt="EXIPTV" draggable={false} />
      </div>
      <ul>
        {ITEMS.map((it) => (
          <li key={it.to}>
            <NavLink
              to={it.to}
              end={it.to === "/"}
              className={({ isActive }) => (isActive ? "active" : "")}
              title={collapsed ? t(it.key) : undefined}
            >
              <span className="icon">{it.icon}</span>
              <span className="label">{t(it.key)}</span>
            </NavLink>
          </li>
        ))}
      </ul>
      <button
        className="collapse-btn"
        onClick={toggle}
        aria-label={collapsed ? t("nav.expand") : t("nav.collapse")}
        title={collapsed ? t("nav.expand") : t("nav.collapse")}
      >
        <IconChevron open={!collapsed} />
      </button>
    </nav>
  );
}
