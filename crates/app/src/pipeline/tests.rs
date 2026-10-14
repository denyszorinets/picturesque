use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
