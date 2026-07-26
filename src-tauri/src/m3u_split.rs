//! Teilt eine geparste `m3u_plus`-Playlist in Live-TV, Filme und Serien auf
//! und speichert sie in die jeweiligen Tabellen. Das ermöglicht getrennte
//! Filme/Serien auch ohne funktionierendes `player_api.php` (Xtream-API).

use exiptv_core::db::{Database, NewEpisode, NewMovie, NewSeason, NewSeriesFull};
use exiptv_core::models::ImportReport;
use exiptv_core::parser::m3u::M3uEntry;
use exiptv_core::parser::m3u_classify::{classify, parse_series_title, StreamKind};
use std::collections::BTreeMap;

/// Ergebnis der Aufteilung.
pub struct SplitResult {
    pub live: Vec<M3uEntry>,
    pub movies: Vec<NewMovie>,
    pub series: Vec<NewSeriesFull>,
}

/// Teilt Einträge nach Typ auf und baut die VOD-Strukturen.
pub fn split_entries(entries: Vec<M3uEntry>) -> SplitResult {
    let mut live = Vec::new();
    let mut movies = Vec::new();
    // Serien werden nach Name gruppiert; je Serie Staffeln→Episoden.
    // BTreeMap hält stabile Reihenfolge.
    let mut series_map: BTreeMap<String, SeriesAccum> = BTreeMap::new();

    for e in entries {
        match classify(&e.url) {
            StreamKind::Live => live.push(e),
            StreamKind::Movie => {
                movies.push(NewMovie {
                    external_id: None,
                    name: e.name.clone(),
                    url: e.url.clone(),
                    category: e.group.clone(),
                    poster_url: e.logo_url.clone(),
                    ..Default::default()
                });
            }
            StreamKind::Series => {
                let (series_name, season_no, episode_no) = parse_series_title(&e.name);
                let acc = series_map.entry(series_name.clone()).or_insert_with(|| SeriesAccum {
                    poster: e.logo_url.clone(),
                    category: e.group.clone(),
                    seasons: BTreeMap::new(),
                });
                if acc.poster.is_none() { acc.poster = e.logo_url.clone(); }
                let s = season_no.unwrap_or(1);
                let ep = acc.seasons.entry(s).or_default();
                ep.push(NewEpisode {
                    number: episode_no.unwrap_or((ep.len() as i64) + 1),
                    name: Some(e.name.clone()),
                    url: e.url.clone(),
                    plot: None,
                    duration_s: None,
                    poster_url: e.logo_url.clone(),
                });
            }
        }
    }

    // Serien-Accumulator in NewSeriesFull umwandeln.
    let series = series_map.into_iter().map(|(name, acc)| {
        let seasons = acc.seasons.into_iter().map(|(num, episodes)| NewSeason {
            number: num,
            name: None,
            episodes,
        }).collect();
        NewSeriesFull {
            external_id: None,
            name,
            category: acc.category,
            poster_url: acc.poster,
            seasons,
            ..Default::default()
        }
    }).collect();

    SplitResult { live, movies, series }
}

struct SeriesAccum {
    poster: Option<String>,
    category: Option<String>,
    seasons: BTreeMap<i64, Vec<NewEpisode>>,
}

/// Speichert das Aufteilungsergebnis (Live/Film/Serie) atomar je Bereich.
/// Leere Teillisten werden übersprungen (z. B. reine VOD-Playlist ohne Live).
pub fn store_split(db: &mut Database, provider_id: i64, split: SplitResult, report: &ImportReport) -> Result<(usize, usize, usize), String> {
    let live_count = split.live.len();
    let movie_count = split.movies.len();
    let series_count = split.series.len();

    if !split.live.is_empty() {
        db.replace_channels_staged(provider_id, &split.live, report).map_err(|e| e.to_string())?;
    }
    // replace_movies/replace_series löschen die alten Daten und fügen neue ein;
    // bei leerer Liste bleibt die Tabelle für den Anbieter einfach leer.
    db.replace_movies(provider_id, &split.movies).map_err(|e| e.to_string())?;
    db.replace_series(provider_id, &split.series).map_err(|e| e.to_string())?;

    Ok((live_count, movie_count, series_count))
}
