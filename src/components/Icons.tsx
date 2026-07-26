// Konsistenter Icon-Satz (Stroke 1.6, 20px-Raster) — bewusst eigenständig,
// keine Fremdbibliothek nötig.
const P = { fill: "none", stroke: "currentColor", strokeWidth: 1.6, strokeLinecap: "round", strokeLinejoin: "round" } as const;
const S = ({ children }: { children: React.ReactNode }) => (
  <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true">{children}</svg>
);

export const IconHome = () => <S><path {...P} d="M4 11 12 4l8 7v8a1 1 0 0 1-1 1h-4v-6h-6v6H5a1 1 0 0 1-1-1z" /></S>;
export const IconLive = () => <S><rect {...P} x="3" y="6" width="18" height="13" rx="2" /><path {...P} d="m9 3 3 3 3-3" /></S>;
export const IconGuide = () => <S><rect {...P} x="3" y="5" width="18" height="15" rx="2" /><path {...P} d="M3 10h18M8 5v15" /></S>;
export const IconMovie = () => <S><rect {...P} x="3" y="4" width="18" height="16" rx="2" /><path {...P} d="M3 9h18M7 4v5M12 4v5M17 4v5" /></S>;
export const IconSeries = () => <S><rect {...P} x="4" y="7" width="16" height="13" rx="2" /><path {...P} d="M7 4h10" /></S>;
export const IconStar = () => <S><path {...P} d="m12 4 2.4 4.9 5.4.8-3.9 3.8.9 5.4L12 16.4 7.2 18.9l.9-5.4L4.2 9.7l5.4-.8z" /></S>;
export const IconRec = () => <S><circle {...P} cx="12" cy="12" r="8" /><circle cx="12" cy="12" r="3.4" fill="currentColor" stroke="none" /></S>;
export const IconHistory = () => <S><circle {...P} cx="12" cy="12" r="8" /><path {...P} d="M12 8v4l3 2" /></S>;
export const IconSearch = () => <S><circle {...P} cx="11" cy="11" r="6" /><path {...P} d="m20 20-4.2-4.2" /></S>;
export const IconServer = () => <S><rect {...P} x="4" y="4" width="16" height="7" rx="2" /><rect {...P} x="4" y="13" width="16" height="7" rx="2" /><path {...P} d="M8 7.5h.01M8 16.5h.01" /></S>;
export const IconGear = () => <S><circle {...P} cx="12" cy="12" r="3.2" /><path {...P} d="M12 3v2.4M12 18.6V21M3 12h2.4M18.6 12H21M5.6 5.6l1.7 1.7M16.7 16.7l1.7 1.7M18.4 5.6l-1.7 1.7M7.3 16.7l-1.7 1.7" /></S>;
export const IconChevron = ({ open }: { open: boolean }) => (
  <S><path {...P} d={open ? "m14 6-6 6 6 6" : "m10 6 6 6-6 6"} /></S>
);
export const IconTv = () => <S><rect {...P} x="4" y="6" width="16" height="11" rx="2" /><path {...P} d="M9 20h6" /></S>;
