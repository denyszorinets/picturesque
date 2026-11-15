use super::*;

/// A `.npy` file with this header dictionary and data.
fn raw_npy(dict: &str, data: &[f32]) -> Vec<u8> {
    let mut header = dict.to_owned();
    // Padded so the data starts at a multiple of 64, as NumPy does.
    while !(10 + header.len() + 1).is_multiple_of(64) {
        header.push(' ');
    }
    header.push('\n');
    let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
    bytes.extend((header.len() as u16).to_le_bytes());
    bytes.extend(header.as_bytes());
    for v in data {
        bytes.extend(v.to_le_bytes());
    }
    bytes
}

/// A `.npy` file of `rows` (C order, little-endian f32).
fn npy(rows: &[&[f32]]) -> Vec<u8> {
    let dict = format!(
        "{{'descr': '<f4', 'fortran_order': False, 'shape': ({}, {}), }}",
        rows.len(),
        rows[0].len()
    );
    raw_npy(&dict, &rows.concat())
}

fn unit(v: &[f32]) -> Embedding {
    Embedding::new(v).unwrap()
}

fn three() -> Classifier {
    Classifier::from_npy(&npy(&[&[1.0, 0.0], &[0.0, 1.0], &[0.6, 0.8]])).unwrap()
}

#[test]
fn reads_a_species_matrix() {
    let c = three();
    assert_eq!((c.species(), c.dim()), (3, 2));
}

#[test]
fn rejects_what_is_not_a_species_matrix() {
    let good = npy(&[&[1.0, 0.0]]);
    // (bytes, part of the error)
    let cases: Vec<(Vec<u8>, &str)> = vec![
        (b"PK\x03\x04 a zip".to_vec(), "magic"),
        (good[..good.len() - 1].to_vec(), "size"),
        (
            raw_npy(
                "{'descr': '<f8', 'fortran_order': False, 'shape': (1, 1), }",
                &[0.0, 0.0],
            ),
            "f32",
        ),
        (
            raw_npy(
                "{'descr': '<f4', 'fortran_order': True, 'shape': (1, 2), }",
                &[0.0, 0.0],
            ),
            "C order",
        ),
        (
            raw_npy(
                "{'descr': '<f4', 'fortran_order': False, 'shape': (2,), }",
                &[0.0, 0.0],
            ),
            "two-dimensional",
        ),
    ];
    for (bytes, why) in cases {
        let err = Classifier::from_npy(&bytes).unwrap_err();
        assert!(err.contains(why), "{why}: {err}");
    }
}

#[test]
fn ranks_species_by_similarity_with_confidences() {
    let ranked = three().rank(&unit(&[0.0, 1.0]), 2, None);

    assert_eq!(ranked.iter().map(|r| r.index).collect::<Vec<_>>(), [1, 2]);
    assert!((ranked[0].similarity - 1.0).abs() < 1e-6);
    assert!(ranked[0].confidence > 0.99, "{ranked:?}");
    assert!(ranked[0].confidence > ranked[1].confidence);
}

#[test]
fn a_region_limits_the_species_and_shares_the_confidence() {
    let c = three();
    let region = HashSet::from([0, 2]);

    let ranked = c.rank(&unit(&[0.0, 1.0]), 5, Some(&region));

    assert_eq!(ranked.iter().map(|r| r.index).collect::<Vec<_>>(), [2, 0]);
    let total: f32 = ranked.iter().map(|r| r.confidence).sum();
    assert!((total - 1.0).abs() < 1e-5);
    assert!(
        c.rank(&unit(&[0.0, 1.0]), 5, Some(&HashSet::new()))
            .is_empty()
    );
    assert!(
        c.rank(&unit(&[1.0, 0.0, 0.0]), 5, None).is_empty(),
        "dimensions differ"
    );
}

#[test]
fn reads_wingclip_label_rows() {
    let row = |a: &str, b: &str, c: &str| vec![a.to_owned(), b.to_owned(), c.to_owned()];
    let labels = labels_from_rows(vec![
        row("American Robin", "Turdus migratorius", "amerob"),
        row("Common Ostrich", "Struthio camelus", "ostric2"),
    ])
    .unwrap();
    assert_eq!(
        labels[0],
        Species {
            common: "American Robin".into(),
            scientific: "Turdus migratorius".into(),
            code: "amerob".into(),
        }
    );
    assert!(labels_from_rows(vec![vec!["only".into(), "two".into()]]).is_err());
}

/// A logit whose sigmoid is `p`.
fn logit(p: f32) -> f32 {
    (p / (1.0 - p)).ln()
}

#[test]
fn decodes_birds_dropping_weak_and_duplicate_boxes_left_to_right() {
    // Three classes; class 1 is "bird". Five queries.
    let low = logit(0.01);
    let logits = [
        low,
        logit(0.9),
        low, // a bird on the right
        low,
        logit(0.8),
        low, // a bird on the left
        low,
        logit(0.7),
        low, // the right bird again, nearly the same box
        low,
        logit(0.2),
        low, // too unsure
        logit(0.95),
        low,
        low, // not a bird
    ];
    let boxes = [
        0.75, 0.5, 0.2, 0.2, //
        0.25, 0.5, 0.2, 0.3, //
        0.76, 0.5, 0.2, 0.2, //
        0.5, 0.5, 0.1, 0.1, //
        0.5, 0.5, 0.5, 0.5,
    ];

    let found = decode_detections(&logits, &boxes, 3, 1, 0.5);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!((found[0].rect.x - 0.15).abs() < 1e-5, "left first");
    assert!((found[0].rect.height - 0.3).abs() < 1e-5);
    assert!((found[0].score - 0.8).abs() < 1e-4);
    assert!((found[1].rect.x - 0.65).abs() < 1e-5);
    assert!(
        (found[1].score - 0.9).abs() < 1e-4,
        "the better duplicate stays"
    );
    assert!(decode_detections(&logits, &boxes, 3, 5, 0.5).is_empty());
    assert!(decode_detections(&[], &[], 0, 0, 0.5).is_empty());
}

#[test]
fn boxes_are_kept_inside_the_image() {
    let found = decode_detections(&[logit(0.9)], &[0.05, 0.95, 0.2, 0.2], 1, 0, 0.5);
    let r = &found[0].rect;
    assert_eq!(r.x, 0.0);
    assert!((r.y - 0.85).abs() < 1e-5);
    assert!((r.width - 0.15).abs() < 1e-5 && (r.height - 0.15).abs() < 1e-5);
}

#[test]
fn detector_input_is_a_640_square_of_shares() {
    let input = detector_input(&crate::pixels::RgbImage::filled(1200, 800, [255, 0, 51]));
    let plane = 640 * 640;
    assert_eq!(input.len(), 3 * plane);
    assert!((input[0] - 1.0).abs() < 1e-6);
    assert!(input[plane].abs() < 1e-6);
    assert!((input[2 * plane] - 0.2).abs() < 1e-6);
}

#[test]
fn crops_a_square_around_the_box_with_some_margin() {
    let rect = |x, y, width, height| Rect {
        x,
        y,
        width,
        height,
    };
    // (rect, image size, expected (x, y, side))
    let cases = [
        // A 100 × 50 box in the middle of 1000 × 1000: 120 square around it.
        (
            rect(0.45, 0.475, 0.1, 0.05),
            (1000, 1000),
            Some((440, 440, 120)),
        ),
        // At the corner: shifted to stay inside.
        (rect(0.0, 0.0, 0.1, 0.1), (1000, 1000), Some((0, 0, 120))),
        // Larger than the image allows: the shorter side.
        (rect(0.0, 0.0, 1.0, 1.0), (600, 400), Some((100, 0, 400))),
        // Outside the image, empty or not a number: nothing.
        (rect(1.5, 0.0, 0.2, 0.2), (100, 100), None),
        (rect(0.2, 0.2, 0.0, 0.3), (100, 100), None),
        (rect(f32::NAN, 0.2, 0.3, 0.3), (100, 100), None),
    ];
    for (r, (w, h), expected) in cases {
        assert_eq!(crop_square(r, w, h), expected, "{r:?}");
    }
}
