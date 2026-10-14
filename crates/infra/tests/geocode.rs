use std::time::Instant;

use picturesque_app::ports::geocoder::ReverseGeocoder;
use picturesque_core::metadata::GeoPoint;
use picturesque_infra::geocode::GeoNamesGeocoder;

fn place(lat: f64, lon: f64) -> Option<(String, String)> {
    GeoNamesGeocoder::default()
        .place(GeoPoint::new(lat, lon).unwrap())
        .map(|p| (p.city, p.country_code))
}

fn city(lat: f64, lon: f64) -> String {
    place(lat, lon).map(|(c, _)| c).unwrap_or_default()
}

#[test]
fn resolves_well_known_places_worldwide() {
    assert_eq!(
        place(38.7223, -9.1393),
        Some(("Lisbon".into(), "PT".into()))
    );
    assert_eq!(place(48.8584, 2.2945).unwrap().1, "FR");
    assert_eq!(city(35.6586, 139.7454), "Tokyo");
    assert_eq!(place(-33.8568, 151.2153).unwrap().1, "AU");
    assert_eq!(city(64.1466, -21.9426), "Reykjavík");
    assert_eq!(place(-18.1416, 178.4419).unwrap().1, "FJ");
    assert_eq!(place(-54.8019, -68.3030).unwrap().1, "AR");
}

#[test]
fn region_and_country_names_are_filled() {
    let p = GeoNamesGeocoder::default()
        .place(GeoPoint::new(40.7128, -74.0060).unwrap())
        .unwrap();

    assert_eq!(p.country, "United States");
    assert_eq!(p.region.as_deref(), Some("New York"));
}

#[test]
fn open_ocean_has_no_place() {
    assert_eq!(place(0.0, -140.0), None);
    assert_eq!(place(-60.0, 20.0), None);
}

#[test]
fn lookups_are_fast_once_loaded() {
    let index = GeoNamesGeocoder::index();
    assert!(index.len() > 200_000);
    let geocoder = GeoNamesGeocoder::default();

    let start = Instant::now();
    for i in 0..1000 {
        let lat = -60.0 + (i as f64) * 0.13;
        let lon = -180.0 + (i as f64) * 0.36;
        let _ = geocoder.place(GeoPoint::new(lat, lon).unwrap());
    }
    let per_lookup = start.elapsed() / 1000;

    assert!(per_lookup.as_micros() < 1000, "{per_lookup:?} per lookup");
}
