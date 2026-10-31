use picturesque_core::metadata::Exposure;

use super::tip_fits;

fn shot(f: f32, mm: f32, iso: u32) -> Exposure {
    Exposure {
        iso: Some(iso),
        f_number: Some(f),
        exposure_time: None,
        focal_length_mm: Some(mm),
    }
}

#[test]
fn tips_that_contradict_the_shot_are_dropped() {
    let wide = shot(4.0, 12.0, 100);
    // (shot, tip, kept)
    let cases = [
        // The reported tip: same aperture and focal length, wrong effect.
        (
            wide,
            "Use f/4.0 at 12mm — a good choice, but consider f/5.6 for a slightly shallower depth of field.",
            false,
        ),
        // The value already used.
        (wide, "Use f/4 for the sharpest result.", false),
        (wide, "Try shooting at 12 mm for more drama.", false),
        (wide, "Use ISO 100 to keep the noise low.", false),
        // A larger f-number does not give a shallower depth of field.
        (
            wide,
            "Stop down to f/8 for a shallower depth of field.",
            false,
        ),
        // A smaller one does not give more.
        (wide, "Open up to f/2.8 for more depth of field.", false),
        (wide, "A wider aperture gives more depth of field.", false),
        (
            wide,
            "A smaller aperture gives a shallower depth of field.",
            false,
        ),
        // Correct advice stays.
        (
            wide,
            "Use f/8 for more depth of field, the whole scene sharp.",
            true,
        ),
        (
            wide,
            "Open up to f/2.8 for a shallower depth of field.",
            true,
        ),
        (
            wide,
            "Raise the ISO to 400 and use 1/500 s to freeze the waves.",
            true,
        ),
        (wide, "Put the horizon on the lower third.", true),
        (wide, "Keep f/4 and focus on the sign.", true),
        // Other lengths and numbers are not mistaken for the shot's.
        (wide, "Zoom to 24 mm to leave out the parked cars.", true),
        (
            shot(1.8, 50.0, 3200),
            "Use ISO 1600 to cut the noise.",
            true,
        ),
        (shot(1.8, 50.0, 3200), "Use f/1.8 and a tripod.", false),
    ];
    for (exposure, tip, kept) in cases {
        assert_eq!(tip_fits(tip, &exposure), kept, "{tip}");
    }
}

#[test]
fn without_shot_data_only_the_wording_is_checked() {
    let none = Exposure::default();
    assert!(tip_fits("Use f/4 at 12 mm.", &none));
    assert!(tip_fits(
        "Open up to f/2.8 for a shallower depth of field.",
        &none
    ));
    // Self-contradictory whatever the shot.
    assert!(!tip_fits(
        "A wider aperture gives more depth of field.",
        &none
    ));
    assert!(!tip_fits(
        "Stop down to f/8 for a shallower depth of field.",
        &none
    ));
}
