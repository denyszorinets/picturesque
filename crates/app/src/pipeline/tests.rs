use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDateTime;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime};

use super::*;
use crate::settings::Settings;
use crate::testing::{FakeMetadata, MemFs, TestWorld};

fn taken(s: &str) -> CaptureMetadata {
    CaptureMetadata {
        captured_at: Some(CaptureTime::new(
            NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap(),
            None,
        )),
        ..Default::default()
    }
}

/// A card with a RAW+JPEG pair and a JPEG; another folder holding a copy
/// of the JPEG.
fn world() -> TestWorld {
    let fs = MemFs::new()
        .dir("/lib")
        .file("/card/DCIM/IMG_1.CR3", b"raw one")
        .file("/card/DCIM/IMG_1.JPG", b"jpg one")
        .file("/card/DCIM/IMG_2.JPG", b"jpg two")
        .file("/backup/old/copy.JPG", b"jpg two");
    let metadata = FakeMetadata {
        by_path: [
            ("/card/DCIM/IMG_1.CR3", taken("2026-10-12 09:00:00")),
            ("/card/DCIM/IMG_1.JPG", taken("2026-10-12 09:00:00")),
            ("/card/DCIM/IMG_2.JPG", taken("2026-10-13 10:00:00")),
            ("/backup/old/copy.JPG", taken("2026-10-13 10:00:00")),
        ]
        .into_iter()
        .map(|(p, m)| (PathBuf::from(p), m))
        .collect(),
        failing: Vec::new(),
    };
    let world = TestWorld::new(fs, metadata);
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            path_template: "{year}/{date}/{name}.{ext}".into(),
            ..Default::default()
        })
        .unwrap();
    world
}

fn scan_all(p: &ImportPipeline) {
    p.scan_now(&HashSet::from([None]));
}

fn stages(p: &ImportPipeline) -> Vec<(String, JobStage)> {
    let mut jobs: Vec<(String, JobStage)> = p
        .jobs(JobFilter::All, 100, None, &|_| (false, [0, 0]))
        .unwrap()
        .into_iter()
        .map(|j| (j.file_name, j.stage))
        .collect();
    jobs.sort_by(|a, b| a.0.cmp(&b.0));
    jobs
}

#[test]
fn imports_a_source_photo_by_photo_into_the_library() {
    let world = world();
    let p = world.pipeline();
    let source = p.add_source(Path::new("/card"), true).unwrap();
    assert!(source.watch);
    scan_all(&p);
    assert_eq!(p.status().queued, 2);

    assert_eq!(p.run_until_idle(), 2);
    assert_eq!(
        stages(&p),
        [
            ("IMG_1.CR3".into(), JobStage::Imported),
            ("IMG_2.JPG".into(), JobStage::Imported)
        ]
    );
    for file in [
        "/lib/2026/2026-10-12/IMG_1.CR3",
        "/lib/2026/2026-10-12/IMG_1.JPG",
        "/lib/2026/2026-10-13/IMG_2.JPG",
    ] {
        assert!(world.fs.exists(Path::new(file)), "{file}");
    }
    assert_eq!(world.catalog.photo_count().unwrap(), 2);
    let status = p.status();
    assert_eq!((status.queued, status.imported, status.failed), (0, 2, 0));
    assert!(status.active.is_none());
    let messages = world.pipeline.messages();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("Found 2 new photos in /card"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m == "IMG_1.CR3: imported to 2026/2026-10-12/IMG_1.CR3")
    );
    // Progress was sent.
    assert!(
        world
            .events
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, AppEvent::PipelineProgress(_)))
    );
}

#[test]
fn rescans_only_queue_new_or_changed_photos_and_skip_duplicates() {
    let world = world();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), true).unwrap();
    scan_all(&p);
    p.run_until_idle();

    // Nothing new: nothing queued.
    scan_all(&p);
    assert_eq!(p.status().queued, 0);

    // A new photo on the card, and a second folder with a copy.
    world
        .fs
        .write(Path::new("/card/DCIM/IMG_3.JPG"), b"jpg three");
    p.add_source(Path::new("/backup"), false).unwrap();
    scan_all(&p);
    assert_eq!(p.run_until_idle(), 2);
    let stages = stages(&p);
    assert!(stages.contains(&("IMG_3.JPG".into(), JobStage::Imported)));
    assert!(stages.contains(&("copy.JPG".into(), JobStage::Duplicate)));
    assert_eq!(world.catalog.photo_count().unwrap(), 3);
    let duplicate = p
        .jobs(JobFilter::Duplicate, 10, None, &|_| (false, [0, 0]))
        .unwrap();
    assert_eq!(
        duplicate[0].error.as_deref(),
        Some("copy.JPG is already in the library")
    );
    assert!(duplicate[0].photo_id.is_some());
}

#[test]
fn waits_for_a_library_folder_then_continues() {
    let world = world();
    world.settings.save_settings(&Settings::default()).unwrap();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), true).unwrap();
    scan_all(&p);
    assert_eq!(p.run_until_idle(), 0);
    let status = p.status();
    assert_eq!(status.queued, 2);
    assert!(status.paused.unwrap().contains("library folder"));

    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(p.run_until_idle(), 2);
    assert!(p.status().paused.is_none());
    assert!(
        world
            .pipeline
            .messages()
            .iter()
            .any(|m| m == "Importing resumed")
    );
}

#[test]
fn resumes_photos_interrupted_by_a_stop() {
    let world = world();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), true).unwrap();
    scan_all(&p);
    // The app stopped while copying the first photo.
    let job = world.pipeline.next_job().unwrap().unwrap();
    world
        .pipeline
        .set_job_stage(job.id, JobStage::Copying, None, None)
        .unwrap();

    let restarted = world.pipeline();
    restarted.resume();
    assert_eq!(restarted.run_until_idle(), 2);
    assert_eq!(world.catalog.photo_count().unwrap(), 2);
    assert!(
        world
            .pipeline
            .messages()
            .iter()
            .any(|m| m.contains("Resuming 1 photos"))
    );
}

#[test]
fn failures_are_logged_and_can_be_retried() {
    let world = world();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), true).unwrap();
    scan_all(&p);
    // The card cannot be read when IMG_2 is copied.
    world.fs.fail_copies_of("/card/DCIM/IMG_2.JPG");
    p.run_until_idle();
    let failed = p
        .jobs(JobFilter::Failed, 10, None, &|_| (false, [0, 0]))
        .unwrap();
    assert_eq!(failed.len(), 1, "{:?}", stages(&p));
    assert_eq!(failed[0].file_name, "IMG_2.JPG");
    assert!(
        failed[0]
            .error
            .as_deref()
            .unwrap()
            .contains("input/output error")
    );
    assert_eq!(p.status().failed, 1);
    assert!(
        world
            .pipeline
            .messages()
            .iter()
            .any(|m| m.starts_with("IMG_2.JPG: "))
    );
    // Unchanged on the card: a rescan does not queue it again.
    scan_all(&p);
    assert_eq!(p.status().queued, 0);

    world.fs.allow_copies_of("/card/DCIM/IMG_2.JPG");
    assert_eq!(p.retry(None).unwrap(), 1);
    assert_eq!(p.run_until_idle(), 1);
    assert!(world.fs.exists(Path::new("/lib/2026/2026-10-13/IMG_2.JPG")));
    assert_eq!(p.status().failed, 0);
}

#[test]
fn sources_are_validated_and_can_be_removed() {
    let world = world();
    let p = world.pipeline();
    let invalid = |path: &str| {
        matches!(
            p.add_source(Path::new(path), true),
            Err(PipelineError::Invalid(_))
        )
    };
    assert!(invalid("card"));
    assert!(invalid("/missing"));
    assert!(invalid("/lib"));
    assert!(invalid("/"));
    let source = p.add_source(Path::new("/card"), true).unwrap();
    assert_eq!(
        p.add_source(Path::new("/card"), false).unwrap().id,
        source.id
    );
    assert!(!p.set_watch(source.id, false).unwrap().watch);
    scan_all(&p);
    p.remove_source(source.id).unwrap();
    assert!(p.sources().unwrap().is_empty());
    // Its waiting photos are dropped.
    assert_eq!(p.status().queued, 0);
}

fn eventually(what: &str, check: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while !check() {
        assert!(std::time::Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn watched_folders_are_imported_when_they_change() {
    let world = world();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), true).unwrap();
    p.add_source(Path::new("/backup"), false).unwrap();
    // Nothing is watched until the pipeline starts.
    assert!(world.watcher.folders().is_empty());
    p.start();
    assert_eq!(world.watcher.folders(), [PathBuf::from("/card")]);
    eventually("the card is imported on start", || {
        world.catalog.photo_count().unwrap() == 2
    });

    world
        .fs
        .write(Path::new("/card/DCIM/IMG_3.JPG"), b"jpg three");
    world.watcher.touch("/card");
    eventually("the new photo is imported", || {
        world.catalog.photo_count().unwrap() == 3
    });
    p.set_watch(1, false).unwrap();
    assert!(world.watcher.folders().is_empty());
}

#[test]
fn a_folder_that_cannot_be_watched_is_reported() {
    let world = world();
    world.watcher.failing.lock().unwrap().push("/card".into());
    let p = world.pipeline();
    p.start();
    p.add_source(Path::new("/card"), true).unwrap();
    assert!(
        world
            .pipeline
            .messages()
            .iter()
            .any(|m| m.starts_with("Cannot watch /card for new photos (too many watches)"))
    );
}

#[test]
fn nothing_is_paused_while_nothing_waits() {
    let world = world();
    world.settings.save_settings(&Settings::default()).unwrap();
    let p = world.pipeline();
    assert_eq!(p.run_until_idle(), 0);
    assert!(p.status().paused.is_none());
    assert!(world.pipeline.messages().is_empty());
}

// ---- Copy safety (ADR-0008), photo by photo ----

/// A world with `files` in `/src`, the library in `/lib`, and `template`.
fn source_world(
    fs: MemFs,
    metadata: FakeMetadata,
    template: &str,
) -> (TestWorld, Arc<ImportPipeline>) {
    let world = TestWorld::new(fs.dir("/lib"), metadata);
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            path_template: template.into(),
            ..Default::default()
        })
        .unwrap();
    let p = world.pipeline();
    p.add_source(Path::new("/src"), false).unwrap();
    scan_all(&p);
    (world, p)
}

fn simple(fs: MemFs) -> (TestWorld, Arc<ImportPipeline>) {
    source_world(
        fs,
        FakeMetadata::default(),
        "{year|Undated}/{date}/{name}.{ext}",
    )
}

fn library_files(world: &TestWorld) -> Vec<String> {
    let mut files: Vec<String> = world
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
        .collect();
    files.sort();
    files
}

fn failure(p: &ImportPipeline) -> String {
    let failed = p
        .jobs(JobFilter::Failed, 10, None, &|_| (false, [0, 0]))
        .unwrap();
    assert_eq!(failed.len(), 1, "{:?}", stages(p));
    failed[0].error.clone().unwrap()
}

#[test]
fn names_never_collide_with_files_in_the_library() {
    let t = taken("2026-10-12 10:00:00");
    let metadata = FakeMetadata {
        by_path: [
            (PathBuf::from("/src/a/IMG_1.CR3"), t.clone()),
            (PathBuf::from("/src/b/IMG_1.JPG"), t),
        ]
        .into_iter()
        .collect(),
        failing: Vec::new(),
    };
    let fs = MemFs::new()
        .file("/lib/2026/2026-10-12/IMG_1.NEF", b"already-here")
        .file("/src/a/IMG_1.CR3", b"raw-a")
        .file("/src/a/IMG_1.JPG", b"jpg-a")
        .file("/src/b/IMG_1.JPG", b"jpg-b");
    let (world, p) = source_world(fs, metadata, "{year}/{date}/{name}.{ext}");
    assert_eq!(p.run_until_idle(), 2);
    // The pair keeps one name; nothing reuses a base name already on disk,
    // so the lone JPEG cannot later look like part of the pair.
    assert_eq!(
        library_files(&world),
        [
            "2026/2026-10-12/IMG_1 (2).CR3",
            "2026/2026-10-12/IMG_1 (2).JPG",
            "2026/2026-10-12/IMG_1 (3).JPG",
            "2026/2026-10-12/IMG_1.NEF",
        ]
    );
}

#[test]
fn a_corrupted_copy_is_retried_once() {
    let (world, p) = simple(MemFs::new().file("/src/a.jpg", b"a"));
    world.fs.corrupt_next_copies(1);
    p.run_until_idle();
    assert_eq!(
        world.fs.read(Path::new("/lib/Undated/a.jpg")).unwrap(),
        b"a"
    );
}

#[test]
fn repeated_corruption_fails_the_photo_and_leaves_nothing() {
    let (world, p) = simple(MemFs::new().file("/src/a.jpg", b"a"));
    world.fs.corrupt_next_copies(2);
    p.run_until_idle();
    assert!(failure(&p).contains("did not match"));
    assert!(library_files(&world).is_empty());
}

#[test]
fn a_photo_whose_copy_fails_is_rolled_back_and_others_continue() {
    let fs = MemFs::new()
        .file("/src/IMG_1.CR3", b"raw")
        .file("/src/IMG_1.JPG", b"jpg")
        .file("/src/IMG_2.JPG", b"two");
    let (world, p) = simple(fs);
    world.fs.fail_copies_of("/src/IMG_1.JPG");
    assert_eq!(p.run_until_idle(), 2);
    assert!(failure(&p).contains("input/output error"));
    // The RAW was copied before the JPEG failed; it is removed again.
    assert_eq!(library_files(&world), ["Undated/IMG_2.JPG"]);
}

#[test]
fn a_photo_changed_after_it_was_found_is_imported_as_it_is_now() {
    let (world, p) = simple(MemFs::new().file("/src/a.jpg", b"a"));
    world.fs.write(Path::new("/src/a.jpg"), b"edited later");
    p.run_until_idle();
    assert!(failure(&p).contains("imported again"));
    assert!(library_files(&world).is_empty());
    // The next scan sees the change and imports the new content.
    scan_all(&p);
    assert_eq!(p.run_until_idle(), 1);
    assert_eq!(
        world.fs.read(Path::new("/lib/Undated/a.jpg")).unwrap(),
        b"edited later"
    );
}

#[test]
fn a_photo_changed_during_its_copy_is_not_kept() {
    let (world, p) = simple(MemFs::new().file("/src/a.jpg", b"a"));
    world.fs.change_after_copy("/src/a.jpg");
    p.run_until_idle();
    failure(&p);
    assert!(library_files(&world).is_empty());
}

#[test]
fn a_copy_left_by_an_interrupted_attempt_is_reused() {
    let (world, p) = simple(MemFs::new().file("/src/a.jpg", b"aaa"));
    // The app stopped after renaming the copy, before cataloguing it.
    world.fs.write(Path::new("/lib/Undated/a.jpg"), b"aaa");
    assert_eq!(p.run_until_idle(), 1);
    assert_eq!(library_files(&world), ["Undated/a.jpg"]);
    assert_eq!(world.catalog.photo_count().unwrap(), 1);
}

#[test]
fn a_different_file_at_the_target_is_never_overwritten() {
    let (world, p) = simple(MemFs::new().file("/src/a.jpg", b"aaa"));
    world
        .fs
        .write(Path::new("/lib/Undated/a.jpg"), b"someone else's file");
    p.run_until_idle();
    assert_eq!(
        world.fs.read(Path::new("/lib/Undated/a.jpg")).unwrap(),
        b"someone else's file"
    );
    // The photo gets another name instead.
    assert_eq!(
        library_files(&world),
        ["Undated/a (2).jpg", "Undated/a.jpg"]
    );
}

#[test]
fn an_unreadable_photo_fails_and_a_readable_jpeg_rescues_its_raw() {
    let mut metadata = FakeMetadata {
        failing: vec!["/src/bad.jpg".into(), "/src/IMG_1.CR3".into()],
        ..Default::default()
    };
    metadata
        .by_path
        .insert("/src/IMG_1.JPG".into(), taken("2026-02-02 08:00:00"));
    let fs = MemFs::new()
        .file("/src/bad.jpg", b"x")
        .file("/src/IMG_1.CR3", b"raw")
        .file("/src/IMG_1.JPG", b"jpg");
    let (world, p) = source_world(fs, metadata, "{year|Undated}/{date}/{name}.{ext}");
    assert_eq!(p.run_until_idle(), 2);
    assert!(failure(&p).contains("corrupt"));
    let failed = p
        .jobs(JobFilter::Failed, 10, None, &|_| (false, [0, 0]))
        .unwrap();
    assert_eq!(failed[0].failed_stage, Some(JobStage::Reading));
    assert_eq!(
        library_files(&world),
        ["2026/2026-02-02/IMG_1.CR3", "2026/2026-02-02/IMG_1.JPG"]
    );
}

#[test]
fn decisions_in_existing_sidecars_are_imported() {
    use picturesque_core::decision::{Decision, Flag, Rating};
    let fs = MemFs::new()
        .file("/src/IMG_1.CR3", b"raw")
        .file("/src/IMG_1.CR3.xmp", b"<x/>")
        .file("/src/IMG_2.JPG", b"jpg");
    let world = TestWorld::new(fs.dir("/lib"), FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    let rejected = Decision {
        flag: Flag::Reject,
        rating: Rating::new(2).unwrap(),
    };
    world
        .sidecars
        .0
        .lock()
        .unwrap()
        .insert(PathBuf::from("/src/IMG_1.CR3.xmp"), rejected);
    let p = world.pipeline();
    p.add_source(Path::new("/src"), false).unwrap();
    scan_all(&p);
    assert_eq!(p.run_until_idle(), 2);
    let decisions: Vec<_> = world
        .catalog
        .all_photos()
        .unwrap()
        .iter()
        .map(|photo| {
            (
                file_name(&photo.assets.primary().path),
                world.catalog.decision(photo.id).unwrap(),
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
    assert!(world.fs.exists(Path::new("/lib/Undated/IMG_1.CR3.xmp")));
}

// ---- Full disks ----

#[test]
fn a_full_library_disk_pauses_importing_until_space_is_freed() {
    let world = world();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), true).unwrap();
    scan_all(&p);
    // Room for the reserve and a little more, not for a photo.
    world.fs.set_space(Some(SPACE_RESERVE + 3));
    assert_eq!(p.run_until_idle(), 0);
    let status = p.status();
    assert_eq!((status.queued, status.failed), (2, 0));
    assert!(status.paused.unwrap().contains("library disk is full"));
    let messages = world.pipeline.messages();
    assert_eq!(
        messages
            .iter()
            .filter(|m| m.starts_with("Importing paused"))
            .count(),
        1,
        "{messages:?}"
    );
    // Retrying while still full logs nothing new.
    p.run_until_idle();
    assert_eq!(world.pipeline.messages().len(), messages.len());

    world.fs.set_space(None);
    assert_eq!(p.run_until_idle(), 2);
    assert!(p.status().paused.is_none());
    assert_eq!(world.catalog.photo_count().unwrap(), 2);
    assert!(
        world
            .pipeline
            .messages()
            .iter()
            .any(|m| m == "Importing resumed")
    );
}

#[test]
fn a_disk_filling_up_during_a_copy_requeues_the_photo() {
    let world = world();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), true).unwrap();
    scan_all(&p);
    // The check passes, but the disk fills up while copying.
    world.fs.fill_during_next_copy();
    assert_eq!(p.run_until_idle(), 0);
    let status = p.status();
    assert_eq!((status.queued, status.failed), (2, 0), "{:?}", stages(&p));
    assert!(status.paused.unwrap().contains("library disk is full"));
    // Nothing half-imported is left behind.
    assert!(world.fs.files_under(Path::new("/lib")).is_empty());

    world.fs.set_space(None);
    assert_eq!(p.run_until_idle(), 2);
    assert_eq!(world.catalog.photo_count().unwrap(), 2);
}

#[test]
fn a_full_catalog_disk_pauses_importing_and_reuses_the_copies() {
    use std::sync::atomic::Ordering;
    let world = world();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), true).unwrap();
    scan_all(&p);
    world.catalog.full.store(true, Ordering::Relaxed);
    assert_eq!(p.run_until_idle(), 0);
    let status = p.status();
    assert_eq!((status.queued, status.failed), (2, 0));
    assert!(status.paused.unwrap().contains("catalog is full"));

    world.catalog.full.store(false, Ordering::Relaxed);
    assert_eq!(p.run_until_idle(), 2);
    assert_eq!(world.catalog.photo_count().unwrap(), 2);
    // The copies made before the catalog filled up were used, not doubled.
    assert_eq!(world.fs.files_under(Path::new("/lib")).len(), 3);
}

#[test]
fn a_job_without_files_fails_instead_of_stopping_the_queue() {
    let world = world();
    let p = world.pipeline();
    let source = p.add_source(Path::new("/card"), false).unwrap();
    world
        .pipeline
        .offer_job(source.id, Path::new("/card/DCIM/GONE.JPG"), &[])
        .unwrap();

    assert_eq!(p.run_until_idle(), 1);
    let failed = p
        .jobs(JobFilter::Failed, 10, None, &|_| (false, [0, 0]))
        .unwrap();
    assert_eq!(failed.len(), 1);
    assert!(failed[0].error.as_deref().unwrap().contains("not found"));
    // The photos still in the folder are imported.
    scan_all(&p);
    assert_eq!(p.run_until_idle(), 2);
}

/// A folder culled in place (ADR-0013): a darktable-style sidecar for the
/// pair, and a JPEG in a subfolder.
fn in_place_world() -> TestWorld {
    let fs = MemFs::new()
        .file("/shoot/IMG_1.CR3", b"raw one")
        .file("/shoot/IMG_1.JPG", b"jpg one")
        .file("/shoot/IMG_1.CR3.xmp", b"<xmp/>")
        .file("/shoot/sub/IMG_2.JPG", b"jpg two");
    let metadata = FakeMetadata {
        by_path: [
            ("/shoot/IMG_1.CR3", taken("2026-10-12 09:00:00")),
            ("/shoot/IMG_1.JPG", taken("2026-10-12 09:00:00")),
            ("/shoot/sub/IMG_2.JPG", taken("2026-10-13 10:00:00")),
        ]
        .into_iter()
        .map(|(p, m)| (PathBuf::from(p), m))
        .collect(),
        failing: Vec::new(),
    };
    let world = TestWorld::new(fs, metadata);
    world.sidecars.0.lock().unwrap().insert(
        "/shoot/IMG_1.CR3.xmp".into(),
        picturesque_core::decision::Decision {
            flag: picturesque_core::decision::Flag::Reject,
            rating: picturesque_core::decision::Rating::new(2).unwrap(),
        },
    );
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/shoot".into()),
            open_folder: Some("/shoot".into()),
            ..Default::default()
        })
        .unwrap();
    world
}

fn asset_paths(world: &TestWorld) -> Vec<String> {
    let mut paths: Vec<String> = world
        .catalog
        .all_photos()
        .unwrap()
        .iter()
        .flat_map(|p| p.assets.iter().map(|a| a.path.display().to_string()))
        .collect();
    paths.sort();
    paths
}

#[test]
fn in_place_catalogs_photos_where_they_are() {
    let world = in_place_world();
    let p = world.pipeline();
    p.add_source(Path::new("/shoot"), true).unwrap();
    scan_all(&p);

    assert_eq!(p.run_until_idle(), 2);

    assert_eq!(
        asset_paths(&world),
        ["IMG_1.CR3", "IMG_1.CR3.xmp", "IMG_1.JPG", "sub/IMG_2.JPG"]
    );
    // Nothing was copied.
    assert!(!world.fs.exists(Path::new("/shoot/2026")));
    // The decision in the existing sidecar was read.
    let id = world.catalog.all_photos().unwrap()[0].id;
    assert_eq!(
        world.catalog.decision(id).unwrap().flag,
        picturesque_core::decision::Flag::Reject
    );
    assert!(
        world
            .pipeline
            .messages()
            .iter()
            .any(|m| m == "IMG_1.CR3: catalogued in place")
    );
}

#[test]
fn in_place_writing_a_sidecar_does_not_import_the_photo_again() {
    let world = in_place_world();
    let p = world.pipeline();
    p.add_source(Path::new("/shoot"), true).unwrap();
    scan_all(&p);
    p.run_until_idle();

    world
        .fs
        .write(Path::new("/shoot/sub/IMG_2.JPG.xmp"), b"<xmp/>");
    scan_all(&p);

    assert_eq!(p.status().queued, 0);
}

#[test]
fn in_place_only_the_open_folder_is_a_source() {
    let world = in_place_world();
    let p = world.pipeline();

    assert!(p.add_source(Path::new("/elsewhere"), true).is_err());
    assert!(p.add_source(Path::new("/shoot"), true).is_ok());
}

#[test]
fn in_place_works_in_a_read_only_folder() {
    let fs = MemFs::new()
        .file("/card/IMG_1.JPG", b"jpg one")
        .read_only("/card");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/card".into()),
            open_folder: Some("/card".into()),
            ..Default::default()
        })
        .unwrap();
    let p = world.pipeline();
    p.add_source(Path::new("/card"), false).unwrap();
    scan_all(&p);

    assert_eq!(p.run_until_idle(), 1);
    assert!(p.status().paused.is_none());
}
