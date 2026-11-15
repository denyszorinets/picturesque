use std::path::PathBuf;
use std::sync::Arc;

use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
use picturesque_core::pixels::RgbImage;
use serde_json::{Value, json};

use crate::api::{Api, ApiErrorCode};
use crate::models::{BIRD_DETECTOR, BIRDS, BIRDS_CLASSIFIER_FILE, BIRDS_LABELS_FILE};
use crate::ports::catalog::PhotoCatalog;
use crate::ports::inference::{
    InferenceEngine, InferenceError, Model, ModelSource, Probe, Provider, ProviderChoice, Tensor,
};
use crate::ports::models::ModelSpec;
use crate::ports::settings::SettingsStore;
use crate::settings::{BirdIdSettings, BirdRegion, Settings};
use crate::testing::{FakeMetadata, MemFs, MemoryModelStore, TestWorld};

/// A `.npy` matrix of `rows`.
fn npy(rows: &[Vec<f32>]) -> Vec<u8> {
    let mut header = format!(
        "{{'descr': '<f4', 'fortran_order': False, 'shape': ({}, {}), }}",
        rows.len(),
        rows[0].len()
    );
    while !(10 + header.len() + 1).is_multiple_of(64) {
        header.push(' ');
    }
    header.push('\n');
    let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
    bytes.extend((header.len() as u16).to_le_bytes());
    bytes.extend(header.as_bytes());
    for row in rows {
        for v in row {
            bytes.extend(v.to_le_bytes());
        }
    }
    bytes
}

fn spec() -> ModelSpec {
    crate::models::catalog()
        .into_iter()
        .find(|m| m.id == BIRDS)
        .unwrap()
}

/// One photo, and Bird ID's model, downloaded unless `installed` is
/// false. The fake engine returns its input as the embedding, and the
/// fake decoder gives uniform grey pixels, so the robin's row (the
/// preprocessed grey) is the perfect match; the jay's is its opposite,
/// the wren's halfway.
fn world(installed: bool) -> (TestWorld, Api, i64) {
    build(installed, false)
}

/// A detector that sees a bird on the right (sure), a bird on the left
/// (less sure) and a person; other models get their input back, like the
/// fake engine.
#[derive(Debug)]
struct Scripted;

#[derive(Debug)]
struct ScriptedModel;

impl InferenceEngine for Scripted {
    fn available(&self) -> Vec<Provider> {
        vec![Provider::Cpu]
    }

    fn load(
        &self,
        _model: ModelSource<'_>,
        _choice: ProviderChoice,
    ) -> Result<Box<dyn Model>, InferenceError> {
        Ok(Box::new(ScriptedModel))
    }

    fn probe(&self, _choice: ProviderChoice) -> Result<Probe, InferenceError> {
        Ok(Probe {
            provider: Provider::Cpu,
            fallback: None,
        })
    }
}

impl Model for ScriptedModel {
    fn provider(&self) -> Provider {
        Provider::Cpu
    }

    fn fallback(&self) -> Option<&str> {
        None
    }

    fn run(&self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, InferenceError> {
        if inputs[0].name != "pixel_values" {
            return Ok(inputs);
        }
        let mut logits = vec![-10.0f32; 300 * 80];
        let mut boxes = vec![0.0f32; 300 * 4];
        // (query, class, logit, box)
        for (q, class, logit, b) in [
            (0, 14, 3.0, [0.7, 0.5, 0.2, 0.2]),
            (1, 14, 1.0, [0.2, 0.5, 0.1, 0.2]),
            (2, 0, 5.0, [0.5, 0.5, 0.3, 0.6]),
        ] {
            logits[q * 80 + class] = logit;
            boxes[q * 4..q * 4 + 4].copy_from_slice(&b);
        }
        Ok(vec![
            Tensor::f32("logits", &[1, 300, 80], logits),
            Tensor::f32("pred_boxes", &[1, 300, 4], boxes),
        ])
    }
}

fn build(installed: bool, detector: bool) -> (TestWorld, Api, i64) {
    let world = TestWorld::new(
        MemFs::new().file("/lib/IMG_1.JPG", b"jpg one"),
        FakeMetadata::default(),
    );
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    let id = world
        .catalog
        .insert_photo(&NewPhoto {
            metadata: Default::default(),
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: PathBuf::from("IMG_1.JPG"),
                size: 7,
                hash: ContentHash::from_bytes([7; 32]),
            }])
            .unwrap(),
        })
        .unwrap();
    let grey = picturesque_core::clip::preprocess(&RgbImage::filled(8, 8, [128, 128, 128]));
    let robin = grey.clone();
    let jay: Vec<f32> = grey.iter().map(|v| -v).collect();
    let wren: Vec<f32> = grey
        .iter()
        .enumerate()
        .map(|(i, v)| if i % 2 == 0 { *v } else { 0.0 })
        .collect();
    let labels = json!([
        ["American Robin", "Turdus migratorius", "amerob"],
        ["Blue Jay", "Cyanocitta cristata", "blujay"],
        ["House Wren", "Troglodytes aedon", "houwre"],
    ])
    .to_string();
    let store = Arc::new(MemoryModelStore::default());
    if installed {
        store.install(
            &spec(),
            &[
                (BIRDS_CLASSIFIER_FILE, &npy(&[robin, jay, wren])),
                (BIRDS_LABELS_FILE, labels.as_bytes()),
            ],
        );
    }
    let finder = crate::models::catalog()
        .into_iter()
        .find(|m| m.id == BIRD_DETECTOR)
        .unwrap();
    let mut context = crate::api::AppContext {
        models: vec![spec(), finder.clone()],
        model_store: store.clone(),
        ..world.context()
    };
    if detector {
        store.install(&finder, &[]);
        context.inference = Arc::new(Scripted);
    }
    let api = Api::new(context).unwrap();
    (world, api, id.0)
}

#[test]
fn finds_every_bird_and_suggests_each_ones_species() {
    let (_, api, id) = build(true, true);

    let found = api.call("find_birds", json!({ "photo_id": id })).unwrap();

    let birds = found["birds"].as_array().unwrap();
    assert_eq!(birds.len(), 2, "the person is not a bird: {found}");
    // Left to right.
    let x = |b: &Value| b["box"]["x"].as_f64().unwrap();
    assert!((x(&birds[0]) - 0.15).abs() < 1e-5);
    assert!((x(&birds[1]) - 0.6).abs() < 1e-5);
    assert!(birds[1]["score"].as_f64().unwrap() > birds[0]["score"].as_f64().unwrap());
    for bird in birds {
        assert_eq!(bird["suggestions"][0]["species"]["code"], "amerob");
        assert_eq!(bird["suggestions"].as_array().unwrap().len(), 3);
    }
}

#[test]
fn finding_birds_needs_both_models() {
    let (_, api, id) = build(true, false);
    let err = api
        .call("find_birds", json!({ "photo_id": id }))
        .unwrap_err();
    assert!(err.message.contains("bird finder"), "{}", err.message);

    let (_, api, id) = build(false, false);
    let err = api
        .call("find_birds", json!({ "photo_id": id }))
        .unwrap_err();
    assert!(err.message.contains("bird identifier"), "{}", err.message);
}

fn identify(api: &Api, id: i64, area: Value) -> Result<Value, crate::api::ApiError> {
    api.call("identify_bird", json!({ "photo_id": id, "box": area }))
}

fn names(result: &Value) -> Vec<&str> {
    result["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["species"]["common"].as_str().unwrap())
        .collect()
}

const BOX: &str = r#"{ "x": 0.4, "y": 0.4, "width": 0.2, "height": 0.1 }"#;

fn area() -> Value {
    serde_json::from_str(BOX).unwrap()
}

#[test]
fn suggests_species_for_the_bird_in_the_box() {
    let (_, api, id) = world(true);

    let result = identify(&api, id, area()).unwrap();

    assert_eq!(names(&result), ["American Robin", "House Wren", "Blue Jay"]);
    let top = &result["suggestions"][0];
    assert_eq!(top["species"]["code"], "amerob");
    assert!(top["confidence"].as_f64().unwrap() > 0.99);
    assert_eq!(result["region"], Value::Null);
    assert_eq!(result["elsewhere"], Value::Null);
}

#[test]
fn says_to_download_the_model_first() {
    let (_, api, id) = world(false);

    let err = identify(&api, id, area()).unwrap_err();

    assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    assert!(err.message.contains("Download"), "{}", err.message);
}

#[test]
fn an_empty_box_or_unknown_photo_is_refused() {
    let (_, api, id) = world(true);

    let err = identify(
        &api,
        id,
        json!({ "x": 0.5, "y": 0.5, "width": 0.0, "height": 0.2 }),
    )
    .unwrap_err();
    assert!(err.message.contains("Draw a box"), "{}", err.message);
    assert_eq!(
        identify(&api, 999, area()).unwrap_err().code,
        ApiErrorCode::NotFound
    );
}

#[test]
fn a_region_limits_the_suggestions_and_names_the_best_match_elsewhere() {
    let (world, api, id) = world(true);
    let mut settings = world.settings.load_settings().unwrap();
    settings.bird_id = BirdIdSettings {
        region: "US-WA".into(),
        ebird_key: "key".into(),
    };
    settings.bird_region = BirdRegion {
        region: "US-WA".into(),
        species: vec!["blujay".into(), "houwre".into(), "notinmodel".into()],
    };
    world.settings.save_settings(&settings).unwrap();

    let result = identify(&api, id, area()).unwrap();

    assert_eq!(names(&result), ["House Wren", "Blue Jay"]);
    assert_eq!(result["region"], "US-WA");
    assert_eq!(result["elsewhere"]["common"], "American Robin");

    // A list for another region does not apply.
    settings.bird_id.region = "US-OR".into();
    world.settings.save_settings(&settings).unwrap();
    let result = identify(&api, id, area()).unwrap();
    assert_eq!(names(&result).len(), 3);
    assert_eq!(result["region"], Value::Null);
}

#[test]
fn fetches_and_keeps_the_species_list_of_the_region() {
    let (world, api, _) = world(true);
    world
        .species_lists
        .regions
        .lock()
        .unwrap()
        .insert("US-WA".into(), vec!["amerob".into(), "stejay".into()]);

    // Without a region or key there is nothing to ask.
    let err = api.call("refresh_bird_region", Value::Null).unwrap_err();
    assert!(err.message.contains("region"), "{}", err.message);
    api.call(
        "update_settings",
        json!({ "bird_id": { "region": " us-wa ", "ebird_key": "" } }),
    )
    .unwrap();
    let err = api.call("refresh_bird_region", Value::Null).unwrap_err();
    assert!(err.message.contains("API key"), "{}", err.message);

    api.call(
        "update_settings",
        json!({ "bird_id": { "region": "US-WA", "ebird_key": "secret" } }),
    )
    .unwrap();
    let result = api.call("refresh_bird_region", Value::Null).unwrap();

    assert_eq!(
        result,
        json!({ "region": "US-WA", "species": 2, "known": 1 })
    );
    assert_eq!(
        world.species_lists.asked.lock().unwrap().last().unwrap(),
        &("US-WA".to_owned(), "secret".to_owned())
    );
    let settings = world.settings.load_settings().unwrap();
    assert_eq!(settings.bird_region.species, ["amerob", "stejay"]);
    assert_eq!(settings.region_species().map(<[String]>::len), Some(2));

    // eBird's refusal is passed on.
    api.call(
        "update_settings",
        json!({ "bird_id": { "region": "XX", "ebird_key": "secret" } }),
    )
    .unwrap();
    let err = api.call("refresh_bird_region", Value::Null).unwrap_err();
    assert!(err.message.contains("XX"), "{}", err.message);
}

#[test]
fn region_codes_are_checked() {
    let (_, api, _) = world(true);

    let err = api
        .call(
            "update_settings",
            json!({ "bird_id": { "region": "US/WA", "ebird_key": "" } }),
        )
        .unwrap_err();

    assert!(
        err.message.contains("not an eBird region code"),
        "{}",
        err.message
    );
}

#[test]
fn keeps_every_species_chosen_for_a_photo() {
    let (world, api, id) = world(true);
    let robin =
        json!({ "common": "American Robin", "scientific": "Turdus migratorius", "code": "amerob" });
    let jay =
        json!({ "common": "Blue Jay", "scientific": "Cyanocitta cristata", "code": "blujay" });

    api.call(
        "set_photo_species",
        json!({ "photo_id": id, "species": [jay, robin, jay] }),
    )
    .unwrap();

    assert_eq!(
        api.call("photo_species", json!({ "photo_ids": [id, 999] }))
            .unwrap(),
        json!([{ "photo_id": id, "species": [jay, robin] }]),
        "in the order chosen, once each"
    );
    api.call(
        "set_photo_species",
        json!({ "photo_id": id, "species": [] }),
    )
    .unwrap();
    assert_eq!(
        api.call("photo_species", json!({ "photo_ids": [id] }))
            .unwrap(),
        json!([])
    );
    // It leaves with the photo.
    api.call(
        "set_photo_species",
        json!({ "photo_id": id, "species": [robin] }),
    )
    .unwrap();
    world
        .catalog
        .remove_photo(picturesque_core::photo::PhotoId(id))
        .unwrap();
    assert!(world.catalog.species.lock().unwrap().is_empty());
}
