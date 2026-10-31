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

#[test]
fn a_blurred_background_is_not_advised_for_scenes_that_need_their_surroundings() {
    let critique = |scene: &str| -> super::Critique {
        serde_json::from_value(serde_json::json!({
            "summary": "A lake under a clear sky.", "rating": 6, "scene": scene,
            "camera_tips": [
                "Use f/2.8 for a shallower depth of field.",
                "Use a wider aperture to blur the background.",
                "Use f/11 so the whole lake is sharp.",
                "Raise the shutter speed to 1/500 s."
            ],
            "improvements": ["Isolate the peak with bokeh.", "Put the horizon on a third."]
        }))
        .unwrap()
    };
    let at_f56 = shot(5.6, 24.0, 100);
    // (scene, camera tips kept, improvements kept)
    let cases = [
        ("landscape", 2, 1),
        ("architecture", 2, 1),
        ("street", 2, 1),
        // Where blur suits the subject, the tips stay.
        ("portrait", 4, 2),
        ("macro", 4, 2),
        ("other", 4, 2),
    ];
    for (scene, tips, improvements) in cases {
        let mut c = critique(scene);
        c.check_against(&at_f56);
        assert_eq!(c.camera_tips.len(), tips, "{scene}");
        assert_eq!(c.improvements.len(), improvements, "{scene}");
    }
    // An unknown scene is not a failed reply and changes nothing.
    let mut c = critique("seascape");
    assert_eq!(c.scene, None);
    c.check_against(&at_f56);
    assert_eq!(c.camera_tips.len(), 4);
}
