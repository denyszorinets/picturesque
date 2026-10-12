//! Offline reverse geocoding with GeoNames (ADR-0007).

mod kdtree;

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, OnceLock};

use flate2::read::GzDecoder;
use picturesque_app::ports::geocoder::ReverseGeocoder;
use picturesque_core::metadata::{GeoPoint, Place};

use kdtree::{KdTree, chord2_to_km, km_to_chord2, to_unit};

/// GeoNames `cities500`, built by `scripts/build_geonames.py`.
const DATASET: &[u8] = include_bytes!("../../data/geonames-cities500.tsv.gz");

/// Places farther than this from a photo are not used to name it.
pub const DEFAULT_MAX_DISTANCE_KM: f64 = 50.0;

/// A place "claims" the area within `CLAIM_KM_PER_SQRT_POP * sqrt(population)`
/// km of it, clamped to `MIN_CLAIM_KM..=MAX_CLAIM_KM`. Among the places
/// whose claim covers a photo, the most populous names it, so a photo in a
/// big city's district is named after the city (Tokyo, not Minato City).
const CLAIM_KM_PER_SQRT_POP: f64 = 0.005;
const MIN_CLAIM_KM: f64 = 2.0;
const MAX_CLAIM_KM: f64 = 20.0;

fn claim_km(population: u32) -> f64 {
    (CLAIM_KM_PER_SQRT_POP * f64::from(population).sqrt()).clamp(MIN_CLAIM_KM, MAX_CLAIM_KM)
}

/// Attribution required by the GeoNames licence.
pub const ATTRIBUTION: &str = "Place names from GeoNames (geonames.org), licensed CC BY 4.0.";

#[derive(Debug)]
struct Entry {
    name: Arc<str>,
    region: Option<Arc<str>>,
    country_code: Arc<str>,
    population: u32,
}

/// An in-memory place index.
#[derive(Debug)]
pub struct PlaceIndex {
    entries: Vec<Entry>,
    countries: HashMap<Arc<str>, Arc<str>>,
    tree: KdTree,
}

impl PlaceIndex {
    /// Parses the dataset format written by `scripts/build_geonames.py`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut countries: HashMap<Arc<str>, Arc<str>> = HashMap::new();
        let mut regions: HashMap<String, Arc<str>> = HashMap::new();
        let mut entries = Vec::new();
        let mut points = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let cols: Vec<&str> = line.split('\t').collect();
            match cols.as_slice() {
                ["C", code, name] => {
                    countries.insert((*code).into(), (*name).into());
                }
                ["P", name, region, cc, lat, lon, population] => {
                    let bad = |what| format!("line {}: invalid {what}", n + 1);
                    let lat: f64 = lat.parse().map_err(|_| bad("latitude"))?;
                    let lon: f64 = lon.parse().map_err(|_| bad("longitude"))?;
                    let population: u32 = population.parse().map_err(|_| bad("population"))?;
                    let country_code = countries
                        .get_key_value(*cc)
                        .map(|(k, _)| k.clone())
                        .ok_or_else(|| bad("country code"))?;
                    let region = (!region.is_empty()).then(|| {
                        regions
                            .entry((*region).to_owned())
                            .or_insert_with(|| (*region).into())
                            .clone()
                    });
                    points.push(to_unit(lat, lon));
                    entries.push(Entry {
                        name: (*name).into(),
                        region,
                        country_code,
                        population,
                    });
                }
                _ if line.starts_with('#') || line.is_empty() => {}
                _ => return Err(format!("line {}: unrecognized record", n + 1)),
            }
        }
        Ok(Self {
            entries,
            countries,
            tree: KdTree::build(points),
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The place that names `point`, with its distance in km: the most
    /// populous place whose claim covers the point, else the nearest place
    /// within `max_km`.
    pub fn nearest(&self, point: GeoPoint, max_km: f64) -> Option<(Place, f64)> {
        let target = to_unit(point.latitude(), point.longitude());
        let claimed = self
            .tree
            .within(&target, km_to_chord2(MAX_CLAIM_KM.min(max_km)))
            .into_iter()
            .map(|(i, chord2)| (i, chord2_to_km(chord2)))
            .filter(|&(i, km)| km <= claim_km(self.entries[i as usize].population))
            .max_by(|a, b| {
                let pop = |i: u32| self.entries[i as usize].population;
                pop(a.0).cmp(&pop(b.0)).then(b.1.total_cmp(&a.1))
            });
        let (index, km) = match claimed {
            Some(hit) => hit,
            None => {
                let (index, chord2) = self.tree.nearest(&target)?;
                (index, chord2_to_km(chord2))
            }
        };
        if km > max_km {
            return None;
        }
        let entry = &self.entries[index as usize];
        let place = Place {
            city: entry.name.to_string(),
            region: entry.region.as_deref().map(str::to_owned),
            country: self.countries[&entry.country_code].to_string(),
            country_code: entry.country_code.to_string(),
        };
        Some((place, km))
    }
}

/// [`ReverseGeocoder`] over the bundled GeoNames dataset. The index (about
/// 225,000 places) is built on first use and shared.
#[derive(Debug, Clone, Copy)]
pub struct GeoNamesGeocoder {
    max_distance_km: f64,
}

impl Default for GeoNamesGeocoder {
    fn default() -> Self {
        Self {
            max_distance_km: DEFAULT_MAX_DISTANCE_KM,
        }
    }
}

impl GeoNamesGeocoder {
    pub fn with_max_distance_km(max_distance_km: f64) -> Self {
        Self { max_distance_km }
    }

    /// The shared index, loading it if needed.
    pub fn index() -> &'static PlaceIndex {
        static INDEX: OnceLock<PlaceIndex> = OnceLock::new();
        INDEX.get_or_init(|| {
            let mut text = String::new();
            GzDecoder::new(DATASET)
                .read_to_string(&mut text)
                .expect("bundled GeoNames dataset is valid gzip");
            PlaceIndex::parse(&text).expect("bundled GeoNames dataset is valid")
        })
    }
}

impl ReverseGeocoder for GeoNamesGeocoder {
    fn place(&self, point: GeoPoint) -> Option<Place> {
        Self::index()
            .nearest(point, self.max_distance_km)
            .map(|(place, _)| place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# sample\n\
        C\tPT\tPortugal\n\
        C\tUS\tUnited States\n\
        P\tLisbon\tLisbon\tPT\t38.7251\t-9.1498\t517802\n\
        P\tAlfama\tLisbon\tPT\t38.7114\t-9.1300\t1000\n\
        P\tPorto\tPorto\tPT\t41.1496\t-8.6110\t249633\n\
        P\tLisbon\tOhio\tUS\t39.8609\t-83.6352\t2700\n\
        P\tNowhere\t\tUS\t40.0\t-100.0\t0\n";

    fn point(lat: f64, lon: f64) -> GeoPoint {
        GeoPoint::new(lat, lon).unwrap()
    }

    #[test]
    fn finds_the_nearest_place_with_region_and_country() {
        let index = PlaceIndex::parse(SAMPLE).unwrap();

        let (place, km) = index.nearest(point(38.7223, -9.1393), 50.0).unwrap();

        assert_eq!(place.city, "Lisbon");
        assert_eq!(place.region.as_deref(), Some("Lisbon"));
        assert_eq!(place.country, "Portugal");
        assert_eq!(place.country_code, "PT");
        assert!(km < 2.0, "{km}");
    }

    #[test]
    fn a_big_city_names_photos_taken_in_its_districts() {
        let index = PlaceIndex::parse(SAMPLE).unwrap();

        // Closest to the Alfama entry, but inside Lisbon's claim.
        let (place, km) = index.nearest(point(38.7115, -9.1301), 50.0).unwrap();

        assert_eq!(place.city, "Lisbon");
        assert!(km > 1.5, "{km}");
    }

    #[test]
    fn small_places_outside_big_claims_keep_their_name() {
        let index = PlaceIndex::parse(SAMPLE).unwrap();

        let (place, _) = index.nearest(point(39.87, -83.64), 50.0).unwrap();

        assert_eq!(place.city, "Lisbon");
        assert_eq!(place.country_code, "US");
    }

    #[test]
    fn same_name_in_another_country_is_resolved_by_position() {
        let index = PlaceIndex::parse(SAMPLE).unwrap();

        let (place, _) = index.nearest(point(39.86, -83.63), 50.0).unwrap();

        assert_eq!(place.country_code, "US");
        assert_eq!(place.region.as_deref(), Some("Ohio"));
    }

    #[test]
    fn empty_region_is_none() {
        let index = PlaceIndex::parse(SAMPLE).unwrap();

        assert_eq!(
            index.nearest(point(40.0, -100.0), 1.0).unwrap().0.region,
            None
        );
    }

    #[test]
    fn nothing_beyond_the_maximum_distance() {
        let index = PlaceIndex::parse(SAMPLE).unwrap();

        assert!(index.nearest(point(30.0, -40.0), 50.0).is_none());
    }

    #[test]
    fn rejects_malformed_records() {
        assert!(
            PlaceIndex::parse("P\tX\t\tZZ\t1\t2\t0\n")
                .unwrap_err()
                .contains("country code")
        );
        assert!(
            PlaceIndex::parse("C\tPT\tPortugal\nP\tX\t\tPT\tnorth\t2\t0\n")
                .unwrap_err()
                .contains("latitude")
        );
        assert!(
            PlaceIndex::parse("garbage\n")
                .unwrap_err()
                .contains("unrecognized")
        );
    }
}
