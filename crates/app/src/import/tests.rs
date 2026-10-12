use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDateTime;
use picturesque_core::metadata::{Camera, CaptureMetadata, CaptureTime, GeoPoint};
use serde_json::{Value, json};

use super::model::{ImportState, PhotoStatus};
use crate::api::Api;
use crate::import::{ImportPlanView, ImportStatus};
use crate::ports::catalog::PhotoCatalog;
use crate::ports::events::AppEvent;
use crate::ports::imports::ImportStore;
use crate::ports::settings::SettingsStore;
use crate::settings::Settings;
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

fn taken(s: &str) -> CaptureMetadata {
    CaptureMetadata {
        captured_at: Some(CaptureTime::new(
            NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap(),
            None,
        )),
        camera: Some(Camera::new("Canon", "Canon EOS R5")),
        ..Default::default()
    }
}

struct Harness {
    world: TestWorld,
    api: Api,
}

impl Harness {
    fn new(fs: MemFs, metadata: &[(&str, CaptureMetadata)]) -> Self {
        Self::with_template(fs, metadata, "{year|Undated}/{date}/{name}.{ext}")
    }

    fn with_template(fs: MemFs, metadata: &[(&str, CaptureMetadata)], template: &str) -> Self {
        let fake = FakeMetadata {
            by_path: metadata
                .iter()
                .map(|(p, m)| (PathBuf::from(p), m.clone()))
                .collect(),
            failing: Vec::new(),
        };
        let world = TestWorld::new(fs.dir("/lib"), fake);
        world
            .settings
            .save_settings(&Settings {
                library_root: Some("/lib".into()),
                path_template: template.into(),
                ..Default::default()
            })
            .unwrap();
        let api = Api::new(world.context()).unwrap();
        Self { world, api }
    }

    fn call(&self, command: &str, args: Value) -> Value {
        self.api
            .call(command, args)
            .unwrap_or_else(|e| panic!("{command}: {e:?}"))
    }

    /// Plans `sources` and waits for planning to finish.
    fn plan(&self, sources: &[&str]) -> ImportPlanView {
        let status = self.call("plan_import", json!({ "sources": sources }));
        let run_id = status["run_id"].as_i64().unwrap();
        self.api.wait_for_imports();
        self.view(run_id)
    }

    fn view(&self, run_id: i64) -> ImportPlanView {
        let view = self.call("get_import", json!({ "run_id": run_id }));
        serde_json::from_value(view).unwrap()
    }

    /// Starts copying and waits for it to finish.
    fn run(&self, run_id: i64) -> ImportPlanView {
        self.call("start_import", json!({ "run_id": run_id }));
        self.api.wait_for_imports();
        self.view(run_id)
    }

    fn library_files(&self) -> Vec<String> {
        self.world
            .fs
            .files_under(Path::new("/lib"))
            .iter()
            .map(|p| {
                p.strip_prefix("/lib")
                    .unwrap()
                    .iter()
                    .map(|c| c.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect()
    }
}

// The view types only implement Serialize for the API; tests read them back.
impl<'de> serde::Deserialize<'de> for ImportPlanView {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = Value::deserialize(d)?;
        let photos = v["photos"]
            .as_array()
            .ok_or_else(|| D::Error::custom("photos"))?
            .iter()
            .map(|p| super::PlannedPhotoView {
                status: serde_json::from_value(p["status"].clone()).unwrap(),
                reason: p["reason"].as_str().map(str::to_owned),
                captured_at: p["captured_at"].as_str().map(str::to_owned),
                place: p["place"].as_str().map(str::to_owned),
                camera: p["camera"].as_str().map(str::to_owned),
                files: p["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| super::PlannedFileView {
                        source: f["source"].as_str().unwrap().to_owned(),
                        target: f["target"].as_str().map(str::to_owned),
                        status: serde_json::from_value(f["status"].clone()).unwrap(),
                        error: f["error"].as_str().map(str::to_owned),
                    })
                    .collect(),
            })
            .collect();
        let s = &v["summary"];
        let n = |k: &str| s[k].as_u64().unwrap() as usize;
        Ok(ImportPlanView {
            run_id: v["run_id"].as_i64().unwrap(),
            state: serde_json::from_value(v["state"].clone()).unwrap(),
            library_root: v["library_root"].as_str().unwrap().to_owned(),
            template: v["template"].as_str().unwrap().to_owned(),
            summary: super::PlanSummary {
                photos: n("photos"),
                to_import: n("to_import"),
                imported: n("imported"),
                duplicates: n("duplicates"),
                failed: n("failed"),
                files: n("files"),
                bytes: s["bytes"].as_u64().unwrap(),
            },
            photos,
            problems: serde_json::from_value(v["problems"].clone()).unwrap(),
            error: v["error"].as_str().map(str::to_owned),
        })
    }
}

fn targets(view: &ImportPlanView) -> Vec<Vec<String>> {
    view.photos
        .iter()
        .map(|p| {
            p.files
                .iter()
                .map(|f| f.target.clone().unwrap_or_default())
                .collect()
        })
        .collect()
}

#[test]
fn plans_targets_without_copying_anything() {
    let fs = MemFs::new()
        .file("/src/IMG_1.CR3", b"raw-1")
        .file("/src/IMG_1.JPG", b"jpg-1")
        .file("/src/IMG_1.CR3.xmp", b"<x/>")
        .file("/src/old/IMG_9.JPG", b"jpg-9");
    let h = Harness::new(
        fs,
        &[
            ("/src/IMG_1.CR3", taken("2026-10-12 14:30:05")),
            ("/src/old/IMG_9.JPG", taken("2019-05-01 09:00:00")),
        ],
    );

    let view = h.plan(&["/src"]);

    assert_eq!(view.state, ImportState::Planned);
    assert_eq!(view.summary.photos, 2);
    assert_eq!(view.summary.to_import, 2);
    assert_eq!(view.summary.files, 4);
    // Capture order: 2019 first.
    assert_eq!(
        targets(&view),
        [
            vec!["2019/2019-05-01/IMG_9.JPG".to_owned()],
            vec![
                "2026/2026-10-12/IMG_1.CR3".to_owned(),
                "2026/2026-10-12/IMG_1.JPG".to_owned(),
                "2026/2026-10-12/IMG_1.CR3.xmp".to_owned(),
            ],
        ]
    );
    assert!(h.library_files().is_empty());
}

#[test]
fn copies_verifies_and_catalogues_photos() {
    let fs = MemFs::new()
        .file("/src/IMG_1.CR3", b"raw-1")
        .file("/src/IMG_1.JPG", b"jpg-1");
    let mut meta = taken("2026-10-12 14:30:05");
    meta.gps = GeoPoint::new(38.72, -9.14);
    let h = Harness::with_template(
        fs,
        &[("/src/IMG_1.CR3", meta)],
        "{year}/{date} {city}/{name}.{ext}",
    );
    let plan = h.plan(&["/src"]);

    let done = h.run(plan.run_id);

    assert_eq!(done.state, ImportState::Completed);
    assert_eq!(done.summary.imported, 1);
    assert_eq!(
        h.library_files(),
        [
            "2026/2026-10-12 Lisbon/IMG_1.CR3",
            "2026/2026-10-12 Lisbon/IMG_1.JPG"
        ]
    );
    assert_eq!(
        h.world
            .fs
            .read(Path::new("/lib/2026/2026-10-12 Lisbon/IMG_1.CR3"))
            .unwrap(),
        b"raw-1"
    );
    // Sources are untouched.
    assert_eq!(
        h.world.fs.read(Path::new("/src/IMG_1.CR3")).unwrap(),
        b"raw-1"
    );

    let photo = h
        .world
        .catalog
        .photo(picturesque_core::photo::PhotoId(1))
        .unwrap()
        .unwrap();
    assert_eq!(photo.metadata.place.unwrap().city, "Lisbon");
    assert_eq!(photo.assets.raw().unwrap().hash, fake_hash(b"raw-1"));
    assert_eq!(
        photo.assets.raw().unwrap().path,
        PathBuf::from_iter(["2026", "2026-10-12 Lisbon", "IMG_1.CR3"])
    );
}

#[test]
fn reports_progress_and_completion_as_events() {
    let fs = MemFs::new()
        .file("/src/a.jpg", b"a")
        .file("/src/b.jpg", b"bb");
    let h = Harness::new(fs, &[]);
    let plan = h.plan(&["/src"]);
    h.run(plan.run_id);

    let events = h.world.events.0.lock().unwrap();
    let statuses: Vec<&ImportStatus> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::ImportProgress(s) => Some(s),
            AppEvent::AnalysisProgress(_) => None,
        })
        .collect();
    assert!(statuses.iter().any(|s| s.state == ImportState::Planning));
    assert!(
        statuses
            .iter()
            .any(|s| s.state == ImportState::Copying && s.done == 2 && s.total == 2)
    );
    let last = statuses.last().unwrap();
    assert_eq!(last.state, ImportState::Completed);
    assert_eq!(last.summary.unwrap().imported, 2);
}

#[test]
fn skips_photos_already_in_the_library() {
    let fs = MemFs::new()
        .file("/src/a.jpg", b"same")
        .file("/src/b.jpg", b"other");
    let h = Harness::new(fs, &[]);
    let first = h.plan(&["/src"]);
    h.run(first.run_id);
    h.world.fs.write(Path::new("/card/copy-of-a.jpg"), b"same");
    h.world.fs.write(Path::new("/card/new.jpg"), b"fresh");

    let second = h.plan(&["/card"]);

    assert_eq!(second.summary.duplicates, 1);
    assert_eq!(second.summary.to_import, 1);
    let dup = second
        .photos
        .iter()
        .find(|p| p.status == PhotoStatus::Duplicate)
        .unwrap();
    assert!(
        dup.reason
            .as_deref()
            .unwrap()
            .contains("already in the library")
    );
}

#[test]
fn identical_files_in_two_sources_are_imported_once() {
    let fs = MemFs::new()
        .file("/a/IMG_1.JPG", b"same-bytes")
        .file("/b/IMG_1 copy.JPG", b"same-bytes")
        .file("/b/other.JPG", b"diff-bytes");
    let h = Harness::new(fs, &[]);

    let plan = h.plan(&["/a", "/b"]);

    assert_eq!(plan.summary.duplicates, 1);
    assert_eq!(plan.summary.to_import, 2);
    let dup = plan
        .photos
        .iter()
        .find(|p| p.status == PhotoStatus::Duplicate)
        .unwrap();
    assert!(
        dup.reason.as_deref().unwrap().contains("identical to"),
        "{:?}",
        dup.reason
    );
}

#[test]
fn avoids_name_collisions_with_the_library_and_within_the_plan() {
    let fs = MemFs::new()
        .file("/lib/2026/2026-10-12/IMG_1.NEF", b"already-here")
        .file("/src/a/IMG_1.CR3", b"raw-a")
        .file("/src/a/IMG_1.JPG", b"jpg-a")
        .file("/src/b/IMG_1.JPG", b"jpg-b");
    let t = taken("2026-10-12 10:00:00");
    let h = Harness::new(
        fs,
        &[("/src/a/IMG_1.CR3", t.clone()), ("/src/b/IMG_1.JPG", t)],
    );

    let plan = h.plan(&["/src"]);

    // The pair keeps one name; nothing reuses a base name already on disk,
    // so the lone JPEG cannot later look like part of the pair.
    assert_eq!(
        targets(&plan),
        [
            vec![
                "2026/2026-10-12/IMG_1 (2).CR3".to_owned(),
                "2026/2026-10-12/IMG_1 (2).JPG".to_owned()
            ],
            vec!["2026/2026-10-12/IMG_1 (3).JPG".to_owned()],
        ]
    );
}

#[test]
fn retries_a_corrupted_copy_once() {
    let fs = MemFs::new().file("/src/a.jpg", b"a");
    let h = Harness::new(fs, &[]);
    let plan = h.plan(&["/src"]);
    h.world.fs.corrupt_next_copies(1);

    let done = h.run(plan.run_id);

    assert_eq!(done.summary.imported, 1);
    assert_eq!(
        h.world.fs.read(Path::new("/lib/Undated/a.jpg")).unwrap(),
        b"a"
    );
}

#[test]
fn a_photo_whose_copy_keeps_failing_is_rolled_back_and_others_continue() {
    let fs = MemFs::new()
        .file("/src/IMG_1.CR3", b"raw")
        .file("/src/IMG_1.JPG", b"jpg")
        .file("/src/IMG_2.JPG", b"two");
    let h = Harness::new(
        fs,
        &[
            ("/src/IMG_1.CR3", taken("2026-01-01 10:00:00")),
            ("/src/IMG_2.JPG", taken("2026-01-01 11:00:00")),
        ],
    );
    let plan = h.plan(&["/src"]);
    h.world.fs.fail_copies_of("/src/IMG_1.JPG");

    let done = h.run(plan.run_id);

    assert_eq!(done.state, ImportState::Completed);
    assert_eq!(done.summary.imported, 1);
    assert_eq!(done.summary.failed, 1);
    let failed = done
        .photos
        .iter()
        .find(|p| p.status == PhotoStatus::Failed)
        .unwrap();
    assert!(
        failed
            .reason
            .as_deref()
            .unwrap()
            .contains("input/output error")
    );
    // The RAW was copied before the JPEG failed; it is removed again.
    assert_eq!(h.library_files(), ["2026/2026-01-01/IMG_2.JPG"]);
}

#[test]
fn repeated_corruption_fails_the_photo() {
    let fs = MemFs::new().file("/src/a.jpg", b"a");
    let h = Harness::new(fs, &[]);
    let plan = h.plan(&["/src"]);
    h.world.fs.corrupt_next_copies(2);

    let done = h.run(plan.run_id);

    assert_eq!(done.summary.failed, 1);
    assert!(
        done.photos[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("did not match")
    );
    assert!(h.library_files().is_empty(), "{:?}", h.library_files());
}

#[test]
fn a_source_changed_after_planning_is_not_copied() {
    let fs = MemFs::new().file("/src/a.jpg", b"a");
    let h = Harness::new(fs, &[]);
    let plan = h.plan(&["/src"]);
    h.world.fs.write(Path::new("/src/a.jpg"), b"edited later");

    let done = h.run(plan.run_id);

    assert_eq!(done.summary.failed, 1);
    assert!(
        done.photos[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("plan the import again")
    );
    assert!(h.library_files().is_empty());
}

#[test]
fn a_source_changed_during_its_copy_is_not_kept() {
    let fs = MemFs::new().file("/src/a.jpg", b"a");
    let h = Harness::new(fs, &[]);
    let plan = h.plan(&["/src"]);
    h.world.fs.change_after_copy("/src/a.jpg");

    let done = h.run(plan.run_id);

    assert_eq!(done.summary.failed, 1);
    assert!(h.library_files().is_empty());
}

#[test]
fn resumes_using_files_copied_by_an_interrupted_attempt() {
    let fs = MemFs::new()
        .file("/src/a.jpg", b"aaa")
        .file("/src/b.jpg", b"bbbb");
    let h = Harness::new(fs, &[]);
    let plan = h.plan(&["/src"]);
    // A previous attempt copied a.jpg but stopped before recording it.
    h.world.fs.write(Path::new("/lib/Undated/a.jpg"), b"aaa");

    let done = h.run(plan.run_id);

    assert_eq!(done.summary.imported, 2);
    assert_eq!(h.library_files(), ["Undated/a.jpg", "Undated/b.jpg"]);
}

#[test]
fn a_different_file_at_the_target_is_never_overwritten() {
    let fs = MemFs::new().file("/src/a.jpg", b"aaa");
    let h = Harness::new(fs, &[]);
    let plan = h.plan(&["/src"]);
    h.world
        .fs
        .write(Path::new("/lib/Undated/a.jpg"), b"someone else's file");

    let done = h.run(plan.run_id);

    assert_eq!(done.summary.failed, 1);
    assert_eq!(
        h.world.fs.read(Path::new("/lib/Undated/a.jpg")).unwrap(),
        b"someone else's file"
    );
}

#[test]
fn cancelled_copy_can_be_resumed() {
    let fs = MemFs::new()
        .file("/src/a.jpg", b"a")
        .file("/src/b.jpg", b"b");
    let h = Harness::new(fs, &[]);
    let plan = h.plan(&["/src"]);
    let deps = h.api_deps();
    let run = h.world.imports.load_run(plan.run_id).unwrap().unwrap();

    let outcome = super::copy::copy_run(
        &deps,
        &run,
        &std::sync::atomic::AtomicBool::new(true),
        &|_| {},
    );

    assert_eq!(outcome, super::copy::CopyOutcome::Cancelled);
    assert!(h.library_files().is_empty());
    h.world
        .imports
        .set_state(plan.run_id, ImportState::Cancelled, None)
        .unwrap();
    assert_eq!(h.run(plan.run_id).summary.imported, 2);
}

#[test]
fn runs_left_running_by_a_previous_session_are_interrupted() {
    let h = Harness::new(MemFs::new().file("/src/a.jpg", b"a"), &[]);
    let plan = h.plan(&["/src"]);
    h.world
        .imports
        .set_state(plan.run_id, ImportState::Copying, None)
        .unwrap();

    let restarted = Api::new(h.world.context()).unwrap();

    let status = restarted
        .call("import_status", json!({ "run_id": plan.run_id }))
        .unwrap();
    assert_eq!(status["state"], "interrupted");
    restarted
        .call("start_import", json!({ "run_id": plan.run_id }))
        .unwrap();
    restarted.wait_for_imports();
    assert_eq!(h.library_files(), ["Undated/a.jpg"]);
}

#[test]
fn unreadable_photos_fail_in_the_plan_and_a_readable_jpeg_rescues_its_raw() {
    let fs = MemFs::new()
        .file("/src/bad.jpg", b"x")
        .file("/src/IMG_1.CR3", b"raw")
        .file("/src/IMG_1.JPG", b"jpg");
    let mut metadata = FakeMetadata {
        failing: vec!["/src/bad.jpg".into(), "/src/IMG_1.CR3".into()],
        ..Default::default()
    };
    metadata
        .by_path
        .insert("/src/IMG_1.JPG".into(), taken("2026-02-02 08:00:00"));
    let world = TestWorld::new(fs.dir("/lib"), metadata);
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    let h = Harness {
        api: Api::new(world.context()).unwrap(),
        world,
    };

    let plan = h.plan(&["/src"]);

    assert_eq!(plan.summary.failed, 1);
    assert_eq!(plan.summary.to_import, 1);
    assert!(
        plan.problems
            .iter()
            .any(|p| p.path == Path::new("/src/bad.jpg") && p.reason.contains("corrupt"))
    );
    let pair = plan
        .photos
        .iter()
        .find(|p| p.status == PhotoStatus::Planned)
        .unwrap();
    assert_eq!(
        pair.files[0].target.as_deref(),
        Some("2026/2026-02-02/IMG_1.CR3")
    );
}

#[test]
fn lists_unreadable_folders_and_orphan_sidecars_as_problems() {
    let fs = MemFs::new()
        .file("/src/ok.jpg", b"o")
        .file("/src/lonely.xmp", b"x")
        .file("/src/locked/a.jpg", b"a")
        .unreadable("/src/locked");
    let h = Harness::new(fs, &[]);

    let plan = h.plan(&["/src"]);

    let reasons: Vec<&str> = plan.problems.iter().map(|p| p.reason.as_str()).collect();
    assert!(
        reasons.iter().any(|r| r.contains("Cannot read folder")),
        "{reasons:?}"
    );
    assert!(
        reasons.iter().any(|r| r.contains("Sidecar without")),
        "{reasons:?}"
    );
}

#[test]
fn rejects_invalid_requests_with_explanations() {
    let h = Harness::new(MemFs::new().file("/src/a.jpg", b"a"), &[]);
    let err = |command: &str, args: Value| h.api.call(command, args).unwrap_err().message;

    assert!(err("plan_import", json!({ "sources": [] })).contains("at least one folder"));
    assert!(err("plan_import", json!({ "sources": ["relative"] })).contains("absolute"));
    assert!(err("plan_import", json!({ "sources": ["/lib/sub"] })).contains("inside the library"));
    assert!(err("import_status", json!({ "run_id": 99 })).contains("no import 99"));
    assert!(err("cancel_import", json!({ "run_id": 99 })).contains("not running"));

    let plan = h.plan(&["/src"]);
    h.run(plan.run_id);
    assert!(err("start_import", json!({ "run_id": plan.run_id })).contains("completed"));
}

#[test]
fn planning_needs_a_library_folder() {
    let world = TestWorld::new(
        MemFs::new().file("/src/a.jpg", b"a"),
        FakeMetadata::default(),
    );
    let api = Api::new(world.context()).unwrap();

    let err = api
        .call("plan_import", json!({ "sources": ["/src"] }))
        .unwrap_err();

    assert!(err.message.contains("Choose a library folder"));
}

impl Harness {
    fn api_deps(&self) -> super::Deps {
        let ctx = self.world.context();
        super::Deps {
            settings: ctx.settings_store,
            catalog: ctx.catalog,
            store: ctx.imports,
            fs: ctx.fs,
            metadata: ctx.metadata,
            geocoder: ctx.geocoder,
            events: Arc::new(crate::ports::events::NoEvents),
            sidecars: ctx.sidecars,
        }
    }
}

#[test]
fn decisions_in_existing_sidecars_are_imported() {
    use picturesque_core::decision::{Decision, Flag, Rating};
    let fs = MemFs::new()
        .file("/src/IMG_1.CR3", b"raw")
        .file("/src/IMG_1.CR3.xmp", b"<x/>")
        .file("/src/IMG_2.JPG", b"jpg");
    let h = Harness::new(fs, &[]);
    let rejected = Decision {
        flag: Flag::Reject,
        rating: Rating::new(2).unwrap(),
    };
    h.world
        .sidecars
        .0
        .lock()
        .unwrap()
        .insert(PathBuf::from("/src/IMG_1.CR3.xmp"), rejected);

    let plan = h.plan(&["/src"]);
    h.run(plan.run_id);

    let decisions: Vec<_> = h
        .world
        .catalog
        .all_photos()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p.assets
                    .primary()
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                h.world.catalog.decision(p.id).unwrap(),
            )
        })
        .collect();
    assert!(
        decisions.contains(&("IMG_1.CR3".to_owned(), rejected)),
        "{decisions:?}"
    );
    assert!(
        decisions.contains(&("IMG_2.JPG".to_owned(), Decision::default())),
        "{decisions:?}"
    );
}
