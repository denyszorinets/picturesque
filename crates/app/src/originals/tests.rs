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

/// A card with a RAW+JPEG pair and a JPEG; a backup folder holding a copy
/// of that JPEG; a folder with a pair of which only the JPEG is a copy.
fn fs() -> MemFs {
    MemFs::new()
        .dir("/lib")
        .file("/card/DCIM/IMG_1.CR3", b"raw one")
        .file("/card/DCIM/IMG_1.JPG", b"jpg one")
        .file("/card/DCIM/IMG_2.JPG", b"jpg two")
        .file("/backup/copy.JPG", b"jpg two")
        .file("/half/H_1.CR3", b"another raw")
        .file("/half/H_1.JPG", b"jpg one")
}

fn world_with(fs: MemFs, delete_after_import: DeleteAfterImport) -> TestWorld {
    let all = [
        ("/card/DCIM/IMG_1.CR3", "2026-10-12 09:00:00"),
        ("/card/DCIM/IMG_1.JPG", "2026-10-12 09:00:00"),
        ("/card/DCIM/IMG_2.JPG", "2026-10-13 10:00:00"),
        ("/card/DCIM/IMG_3.JPG", "2026-10-14 10:00:00"),
        ("/backup/copy.JPG", "2026-10-13 10:00:00"),
        ("/half/H_1.CR3", "2026-10-12 09:00:00"),
        ("/half/H_1.JPG", "2026-10-12 09:00:00"),
    ];
    let metadata = FakeMetadata {
        by_path: all
            .into_iter()
            .map(|(p, t)| (PathBuf::from(p), taken(t)))
            .collect(),
        failing: Vec::new(),
    };
    let world = TestWorld::new(fs, metadata);
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            path_template: "{year}/{date}/{name}.{ext}".into(),
            delete_after_import,
            ..Default::default()
        })
        .unwrap();
    world
}

fn service(world: &TestWorld) -> Arc<OriginalsService> {
    Arc::new(OriginalsService::new(
        world.pipeline.clone(),
        world.catalog.clone(),
        world.fs.clone(),
        world.settings.clone(),
        world.events.clone(),
    ))
}

/// Imports a folder and returns its id.
fn import(world: &TestWorld, folder: &str) -> i64 {
    let pipeline = world.pipeline();
    let source = pipeline.add_source(Path::new(folder), true).unwrap();
    pipeline.scan_now(&HashSet::from([Some(source.id)]));
    pipeline.run_until_idle();
    source.id
}

fn exists(world: &TestWorld, path: &str) -> bool {
    world.fs.exists(Path::new(path))
}

/// The card imported with deleting switched on.
fn card() -> (TestWorld, Arc<OriginalsService>, i64) {
    let world = world_with(fs(), DeleteAfterImport::Ask);
    let id = import(&world, "/card");
    let svc = service(&world);
    (world, svc, id)
}

#[test]
fn nothing_is_offered_unless_the_setting_or_the_folder_asks() {
    // (setting, folder's own choice, offered)
    let cases = [
        (DeleteAfterImport::Never, None, false),
        (DeleteAfterImport::Ask, None, true),
        (DeleteAfterImport::Never, Some(DeleteAfterImport::Ask), true),
        (
            DeleteAfterImport::Ask,
            Some(DeleteAfterImport::Never),
            false,
        ),
        // Leftovers of a folder that deletes right away are offered too.
        (
            DeleteAfterImport::Never,
            Some(DeleteAfterImport::Immediately),
            true,
        ),
    ];
    for (setting, choice, offered) in cases {
        let world = world_with(fs(), setting);
        let id = import(&world, "/card");
        world
            .pipeline
            .set_source_delete_originals(id, choice)
            .unwrap();
        let svc = service(&world);

        let offers = svc.offers().unwrap();

        assert_eq!(
            offers
                .iter()
                .map(|o| (o.source_id, o.photos))
                .collect::<Vec<_>>(),
            if offered { vec![(id, 2)] } else { vec![] },
            "{setting:?} {choice:?}"
        );
        if !offered {
            assert!(matches!(svc.plan(id), Err(OriginalsError::Invalid(_))));
            assert!(matches!(
                svc.run_now(id, 99),
                Err(OriginalsError::Invalid(_))
            ));
            assert!(exists(&world, "/card/DCIM/IMG_1.CR3"));
        }
    }
}

#[test]
fn deleting_removes_the_verified_originals_and_keeps_the_library() {
    let (world, svc, id) = card();

    let plan = svc.plan(id).unwrap();
    assert_eq!(
        (
            plan.photos,
            plan.files,
            plan.bytes,
            plan.duplicates,
            plan.changed
        ),
        (2, 3, 21, 0, 0)
    );
    let status = svc.run_now(id, plan.through_job_id).unwrap();

    assert_eq!(status.state, OriginalsState::Completed);
    assert_eq!(
        (
            status.deleted,
            status.files,
            status.bytes,
            status.done,
            status.total
        ),
        (2, 3, 21, 2, 2)
    );
    assert!(status.kept.is_empty());
    for gone in ["IMG_1.CR3", "IMG_1.JPG", "IMG_2.JPG"] {
        assert!(!exists(&world, &format!("/card/DCIM/{gone}")), "{gone}");
    }
    for kept in [
        "/lib/2026/2026-10-12/IMG_1.CR3",
        "/lib/2026/2026-10-12/IMG_1.JPG",
        "/lib/2026/2026-10-13/IMG_2.JPG",
    ] {
        assert!(exists(&world, kept), "{kept}");
    }
    assert_eq!(world.catalog.photo_count().unwrap(), 2);
    // Done: nothing is offered again, and a second run has nothing to do.
    assert!(svc.offers().unwrap().is_empty());
    assert_eq!(svc.run_now(id, plan.through_job_id).unwrap().total, 0);
}

#[test]
fn an_original_that_changed_is_kept() {
    // The same answer whether it changes before or after the plan; the plan
    // only counts it when it changed before.
    for after_plan in [false, true] {
        let (world, svc, id) = card();
        let change = || {
            world
                .fs
                .write(Path::new("/card/DCIM/IMG_2.JPG"), b"jpg 222")
        };
        if !after_plan {
            change();
        }
        let plan = svc.plan(id).unwrap();
        if after_plan {
            change();
        }

        let status = svc.run_now(id, plan.through_job_id).unwrap();

        let case = format!("after_plan {after_plan}");
        assert_eq!(plan.changed, usize::from(!after_plan), "{case}");
        assert_eq!(status.deleted, 1, "{case}");
        assert!(exists(&world, "/card/DCIM/IMG_2.JPG"), "{case}");
        assert!(!exists(&world, "/card/DCIM/IMG_1.CR3"), "{case}");
        assert_eq!(status.kept.len(), 1, "{case}");
        assert_eq!(
            status.kept[0].reason, "IMG_2.JPG changed since it was imported",
            "{case}"
        );
    }
}

#[test]
fn a_file_that_was_only_touched_is_kept_like_the_plan_says() {
    // Same bytes, new modification time: the plan counts it as changed and
    // the run keeps it, so the dialog's numbers are what happens.
    let (world, svc, id) = card();
    world
        .fs
        .write(Path::new("/card/DCIM/IMG_2.JPG"), b"jpg two");

    let plan = svc.plan(id).unwrap();
    let status = svc.run_now(id, plan.through_job_id).unwrap();

    assert_eq!((plan.photos, plan.changed), (1, 1));
    assert_eq!(
        (status.deleted, status.kept.len()),
        (plan.photos, plan.changed)
    );
    assert!(exists(&world, "/card/DCIM/IMG_2.JPG"));
}

#[test]
fn a_library_copy_that_is_altered_or_missing_keeps_the_original() {
    // (what happens to the library copy, reason)
    type Damage = fn(&MemFs);
    let cases: [(Damage, &str); 2] = [
        (
            |fs| fs.write(Path::new("/lib/2026/2026-10-13/IMG_2.JPG"), b"damaged"),
            "the library copy of IMG_2.JPG changed",
        ),
        (
            |fs| {
                fs.remove_file(Path::new("/lib/2026/2026-10-13/IMG_2.JPG"))
                    .unwrap()
            },
            "the library copy of IMG_2.JPG is missing",
        ),
    ];
    for (damage, reason) in cases {
        let (world, svc, id) = card();
        damage(&world.fs);

        let status = svc.run_now(id, 99).unwrap();

        assert_eq!(
            status
                .kept
                .iter()
                .map(|k| k.reason.as_str())
                .collect::<Vec<_>>(),
            [reason]
        );
        assert!(exists(&world, "/card/DCIM/IMG_2.JPG"), "{reason}");
        assert_eq!(status.deleted, 1, "{reason}");
    }
}

#[test]
fn a_photo_that_cannot_be_fully_deleted_keeps_the_rest_and_is_offered_again() {
    let world = world_with(
        fs().read_only("/card/DCIM/IMG_1.JPG"),
        DeleteAfterImport::Ask,
    );
    let id = import(&world, "/card");
    let svc = service(&world);

    let status = svc.run_now(id, 99).unwrap();

    assert_eq!(status.deleted, 1);
    assert_eq!(status.kept.len(), 1);
    assert!(
        status.kept[0]
            .reason
            .starts_with("cannot delete IMG_1.JPG: read-only file system; 1 of its files"),
        "{}",
        status.kept[0].reason
    );
    // The RAW went first, the JPEG stays, so the photo is still offered.
    assert!(!exists(&world, "/card/DCIM/IMG_1.CR3"));
    assert!(exists(&world, "/card/DCIM/IMG_1.JPG"));
    assert_eq!(svc.offers().unwrap()[0].photos, 1);
}

#[test]
fn a_photo_removed_from_the_library_keeps_its_originals() {
    let (world, svc, id) = card();
    let photo = world.catalog.all_photos().unwrap()[0].id;
    world.catalog.remove_photo(photo).unwrap();

    let status = svc.run_now(id, 99).unwrap();

    assert_eq!(status.kept.len(), 1);
    assert_eq!(
        status.kept[0].reason,
        "the photo is no longer in the library"
    );
    assert_eq!(status.deleted, 1);
}

#[test]
fn duplicates_are_deleted_only_when_every_file_is_in_the_library() {
    let world = world_with(fs(), DeleteAfterImport::Ask);
    import(&world, "/card");
    let backup = import(&world, "/backup");
    let half = import(&world, "/half");
    let svc = service(&world);

    let plan = svc.plan(backup).unwrap();
    assert_eq!((plan.photos, plan.duplicates), (1, 1));
    let backup_status = svc.run_now(backup, plan.through_job_id).unwrap();
    let half_status = svc.run_now(half, 99).unwrap();

    assert_eq!(backup_status.deleted, 1);
    assert!(!exists(&world, "/backup/copy.JPG"));
    // The pair's JPEG is a copy of IMG_1.JPG but its RAW is not in the library.
    assert_eq!(half_status.deleted, 0);
    assert_eq!(half_status.kept[0].reason, "H_1.CR3 is not in the library");
    assert!(exists(&world, "/half/H_1.CR3") && exists(&world, "/half/H_1.JPG"));
}

#[test]
fn photos_imported_after_the_plan_are_not_deleted() {
    let (world, svc, id) = card();
    let plan = svc.plan(id).unwrap();
    world
        .fs
        .write(Path::new("/card/DCIM/IMG_3.JPG"), b"jpg three");
    let pipeline = world.pipeline();
    pipeline.scan_now(&HashSet::from([Some(id)]));
    // Still queued: not offered, not deletable.
    assert!(svc.offers().unwrap().is_empty());
    assert!(matches!(
        svc.run_now(id, plan.through_job_id),
        Err(OriginalsError::Invalid(_))
    ));
    pipeline.run_until_idle();

    let status = svc.run_now(id, plan.through_job_id).unwrap();

    assert_eq!(status.deleted, 2);
    assert!(exists(&world, "/card/DCIM/IMG_3.JPG"));
    assert_eq!(svc.offers().unwrap()[0].photos, 1);
}

#[test]
fn cancelling_stops_between_photos_and_a_new_run_finishes() {
    let (world, svc, id) = card();
    let (source, jobs) = svc.begin(id, 99).unwrap();
    svc.cancel();

    svc.run(&source, &jobs);

    assert_eq!(svc.status().state, OriginalsState::Cancelled);
    assert!(exists(&world, "/card/DCIM/IMG_1.CR3") && exists(&world, "/card/DCIM/IMG_2.JPG"));
    let status = svc.run_now(id, 99).unwrap();
    assert_eq!(
        (status.state, status.deleted),
        (OriginalsState::Completed, 2)
    );
}

#[test]
fn originals_already_deleted_by_hand_are_recorded_as_done() {
    let (world, svc, id) = card();
    world
        .fs
        .remove_file(Path::new("/card/DCIM/IMG_2.JPG"))
        .unwrap();

    let plan = svc.plan(id).unwrap();

    assert_eq!(plan.photos, 1);
    assert_eq!(svc.offers().unwrap()[0].photos, 1);
}

#[test]
fn a_locked_card_and_a_folder_culled_in_place_are_refused() {
    let (world, svc, id) = card();
    world.fs.lock_folder("/card");
    let Err(OriginalsError::Invalid(why)) = svc.plan(id) else {
        panic!("a locked card must be refused");
    };
    assert!(why.contains("cannot be cleared"), "{why}");

    let (world, svc, id) = card();
    let mut settings = world.settings.load_settings().unwrap();
    settings.open_folder = Some("/card".into());
    world.settings.save_settings(&settings).unwrap();
    assert!(svc.offers().unwrap().is_empty());
    assert!(matches!(svc.plan(id), Err(OriginalsError::Invalid(_))));
}

#[test]
fn a_run_in_progress_refuses_another() {
    let (_world, svc, id) = card();
    svc.begin(id, 99).unwrap();
    assert_eq!(svc.run_now(id, 99).unwrap_err(), OriginalsError::Busy);
}

#[test]
fn a_sidecar_edited_since_the_import_keeps_the_whole_photo() {
    // (edited after the import, photo deleted)
    for (edited, deleted) in [(false, true), (true, false)] {
        let world = world_with(
            fs().file("/card/DCIM/IMG_2.xmp", b"<x/>"),
            DeleteAfterImport::Ask,
        );
        let id = import(&world, "/card");
        let svc = service(&world);
        assert_eq!(
            svc.plan(id).unwrap().files,
            4,
            "the sidecar is part of the photo"
        );
        if edited {
            world
                .fs
                .write(Path::new("/card/DCIM/IMG_2.xmp"), b"<x edited/>");
        }

        let status = svc.run_now(id, 99).unwrap();

        assert_eq!(
            !exists(&world, "/card/DCIM/IMG_2.JPG"),
            deleted,
            "edited {edited}"
        );
        assert_eq!(
            !exists(&world, "/card/DCIM/IMG_2.xmp"),
            deleted,
            "edited {edited}"
        );
        if edited {
            assert_eq!(
                status.kept[0].reason,
                "IMG_2.xmp changed since it was imported"
            );
        }
    }
}

/// A finished job for `photo` with one file at an arbitrary path, as the
/// catalog could hold it.
fn job_with_file(world: &TestWorld, source_id: i64, path: &str, bytes: &[u8]) -> i64 {
    use crate::ports::pipeline::{JobFile, PipelineStore};
    world.fs.write(Path::new(path), bytes);
    let stamp = world.fs.stamp(Path::new(path)).unwrap();
    let modified_ns = stamp
        .modified
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_nanos()).ok());
    let file = JobFile {
        path: path.into(),
        format: picturesque_core::media::MediaFormat::Jpeg,
        size: bytes.len() as u64,
        modified_ns,
    };
    let job = world
        .pipeline
        .offer_job(source_id, Path::new(path), &[file])
        .unwrap()
        .unwrap();
    let photo = world.catalog.all_photos().unwrap()[0].id;
    world
        .pipeline
        .set_job_stage(job, JobStage::Imported, Some(photo), None)
        .unwrap();
    job
}

#[test]
fn only_files_inside_the_import_folder_are_deleted() {
    // (path, bytes of the first library photo's JPEG, deleted)
    for path in ["/card/../outside/X.JPG", "/elsewhere/X.JPG"] {
        let (world, svc, id) = card();
        // Make the file's bytes match a library file, so only its place
        // can keep it.
        let library = world.catalog.all_photos().unwrap();
        let asset = library[0].assets.iter().next().unwrap().clone();
        let bytes = world.fs.read(&Path::new("/lib").join(&asset.path)).unwrap();
        let job = job_with_file(&world, id, path, &bytes);

        let status = svc.run_now(id, job).unwrap();

        assert!(
            status
                .kept
                .iter()
                .any(|k| k.reason == "X.JPG is outside the folder"),
            "{path}: {status:?}"
        );
        assert!(exists(&world, path), "{path}");
    }
}

#[test]
fn a_library_that_overlaps_the_import_folder_refuses_everything() {
    // The library folder moved into (or above) the import folder, so an
    // original could be the library's own copy.
    for root in ["/card/library", "/"] {
        let (world, svc, id) = card();
        let mut settings = world.settings.load_settings().unwrap();
        settings.library_root = Some(root.into());
        world.settings.save_settings(&settings).unwrap();

        let Err(OriginalsError::Invalid(why)) = svc.plan(id) else {
            panic!("{root} must be refused");
        };

        assert!(why.contains("overlap"), "{why}");
        assert!(matches!(
            svc.run_now(id, 99),
            Err(OriginalsError::Invalid(_))
        ));
        assert!(exists(&world, "/card/DCIM/IMG_1.CR3"), "{root}");
    }
}

#[test]
fn a_photo_imported_again_after_the_plan_is_kept() {
    let (world, svc, id) = card();
    let (source, jobs) = svc.begin(id, 99).unwrap();
    // The file is replaced and found again: its job goes back in the queue.
    world
        .fs
        .write(Path::new("/card/DCIM/IMG_2.JPG"), b"new one");
    world.pipeline().scan_now(&HashSet::from([Some(id)]));

    svc.run(&source, &jobs);

    let status = svc.status();
    assert_eq!(status.deleted, 1);
    assert_eq!(
        status.kept[0].reason,
        "it was imported again since the plan"
    );
    assert!(exists(&world, "/card/DCIM/IMG_2.JPG"));
}

#[test]
fn removing_the_folder_stops_a_run() {
    use crate::ports::pipeline::PipelineStore;
    let (world, svc, id) = card();
    let (source, jobs) = svc.begin(id, 99).unwrap();
    world.pipeline.remove_source(id).unwrap();

    svc.run(&source, &jobs);

    assert_eq!(svc.status().state, OriginalsState::Cancelled);
    assert!(exists(&world, "/card/DCIM/IMG_1.CR3") && exists(&world, "/card/DCIM/IMG_2.JPG"));
}

#[test]
fn originals_deleted_by_hand_are_counted_apart() {
    let (world, svc, id) = card();
    let (source, jobs) = svc.begin(id, 99).unwrap();
    world
        .fs
        .remove_file(Path::new("/card/DCIM/IMG_2.JPG"))
        .unwrap();

    svc.run(&source, &jobs);

    let status = svc.status();
    assert_eq!((status.deleted, status.gone), (1, 1));
}

/// A pipeline that deletes through the service, as the app wires it.
fn pipeline_deleting(world: &TestWorld) -> Arc<crate::pipeline::ImportPipeline> {
    let pipeline = world.pipeline();
    let svc = service(world);
    pipeline.after_import(move |job| svc.after_import(job));
    pipeline
}

fn import_deleting(world: &TestWorld, folder: &str) -> i64 {
    let pipeline = pipeline_deleting(world);
    let source = pipeline.add_source(Path::new(folder), true).unwrap();
    pipeline.scan_now(&HashSet::from([Some(source.id)]));
    pipeline.run_until_idle();
    source.id
}

fn warnings(world: &TestWorld) -> Vec<String> {
    world
        .pipeline()
        .log_entries(crate::ports::pipeline::LogLevel::Warn, 50, None)
        .unwrap()
        .into_iter()
        .map(|e| e.message)
        .collect()
}

#[test]
fn a_folder_that_deletes_right_away_frees_each_original_once_it_is_imported() {
    let world = world_with(fs(), DeleteAfterImport::Immediately);

    let id = import_deleting(&world, "/card");

    for gone in ["IMG_1.CR3", "IMG_1.JPG", "IMG_2.JPG"] {
        assert!(!exists(&world, &format!("/card/DCIM/{gone}")), "{gone}");
    }
    for kept in [
        "/lib/2026/2026-10-12/IMG_1.CR3",
        "/lib/2026/2026-10-12/IMG_1.JPG",
        "/lib/2026/2026-10-13/IMG_2.JPG",
    ] {
        assert!(exists(&world, kept), "{kept}");
    }
    assert_eq!(world.catalog.photo_count().unwrap(), 2);
    // Recorded as done, so nothing is offered afterwards.
    assert!(
        service(&world)
            .offers()
            .unwrap()
            .iter()
            .all(|o| o.source_id != id)
    );
    assert!(warnings(&world).is_empty());
}

#[test]
fn only_a_folder_set_to_immediately_deletes_during_the_import() {
    // (setting, folder's own choice, originals survive)
    let cases = [
        (DeleteAfterImport::Never, None, true),
        (DeleteAfterImport::Ask, None, true),
        (DeleteAfterImport::Immediately, None, false),
        (
            DeleteAfterImport::Immediately,
            Some(DeleteAfterImport::Never),
            true,
        ),
        (
            DeleteAfterImport::Immediately,
            Some(DeleteAfterImport::Ask),
            true,
        ),
        (
            DeleteAfterImport::Never,
            Some(DeleteAfterImport::Immediately),
            false,
        ),
    ];
    for (setting, choice, survive) in cases {
        let world = world_with(fs(), setting);
        let pipeline = pipeline_deleting(&world);
        let source = pipeline.add_source(Path::new("/card"), true).unwrap();
        world
            .pipeline
            .set_source_delete_originals(source.id, choice)
            .unwrap();
        pipeline.scan_now(&HashSet::from([Some(source.id)]));
        pipeline.run_until_idle();

        assert_eq!(
            exists(&world, "/card/DCIM/IMG_1.CR3") && exists(&world, "/card/DCIM/IMG_2.JPG"),
            survive,
            "{setting:?} {choice:?}"
        );
        assert_eq!(world.catalog.photo_count().unwrap(), 2);
    }
}

#[test]
fn a_photo_that_failed_to_import_keeps_its_original() {
    let mut metadata = FakeMetadata::default();
    metadata.failing.push(PathBuf::from("/card/DCIM/IMG_2.JPG"));
    let world = TestWorld::new(fs(), metadata);
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            delete_after_import: DeleteAfterImport::Immediately,
            ..Default::default()
        })
        .unwrap();

    import_deleting(&world, "/card");

    assert!(exists(&world, "/card/DCIM/IMG_2.JPG"));
}

#[test]
fn a_duplicate_is_deleted_when_the_library_has_the_same_file() {
    let world = world_with(fs(), DeleteAfterImport::Never);
    import(&world, "/card");
    let backup = pipeline_deleting(&world)
        .add_source(Path::new("/backup"), true)
        .unwrap();
    world
        .pipeline
        .set_source_delete_originals(backup.id, Some(DeleteAfterImport::Immediately))
        .unwrap();

    let pipeline = pipeline_deleting(&world);
    pipeline.scan_now(&HashSet::from([Some(backup.id)]));
    pipeline.run_until_idle();

    // The duplicate of IMG_2.JPG is gone; the card, set to never, is intact.
    assert!(!exists(&world, "/backup/copy.JPG"));
    assert!(exists(&world, "/card/DCIM/IMG_2.JPG"));
    assert!(exists(&world, "/lib/2026/2026-10-13/IMG_2.JPG"));
}

#[test]
fn an_original_changed_since_the_import_stays_and_is_reported_once() {
    let world = world_with(fs(), DeleteAfterImport::Ask);
    let id = import(&world, "/card");
    world
        .fs
        .write(Path::new("/card/DCIM/IMG_2.JPG"), b"edited since");
    world
        .pipeline
        .set_source_delete_originals(id, Some(DeleteAfterImport::Immediately))
        .unwrap();
    let svc = service(&world);
    let jobs = world.pipeline.cleanup_candidates(id, None).unwrap();

    let outcomes: Vec<AfterImport> = jobs.iter().map(|j| svc.after_import(j.id)).collect();

    assert!(matches!(
        outcomes[..],
        [AfterImport::Deleted, AfterImport::Kept(_)]
    ));
    assert!(exists(&world, "/card/DCIM/IMG_2.JPG"));
    assert!(!exists(&world, "/card/DCIM/IMG_1.CR3"));
}

#[test]
fn a_locked_folder_is_reported_once_not_for_every_photo() {
    let world = world_with(fs(), DeleteAfterImport::Immediately);
    world.fs.lock_folder("/card");

    import_deleting(&world, "/card");

    assert!(exists(&world, "/card/DCIM/IMG_1.CR3") && exists(&world, "/card/DCIM/IMG_2.JPG"));
    let kept: Vec<String> = warnings(&world)
        .into_iter()
        .filter(|m| m.contains("original kept"))
        .collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(world.catalog.photo_count().unwrap(), 2);
    // Looked at once, not read twice per photo for nothing.
    assert!(world.fs.synced().is_empty());
}

#[test]
fn a_library_moved_inside_the_import_folder_never_deletes() {
    let world = world_with(fs().dir("/card/out"), DeleteAfterImport::Immediately);
    let pipeline = pipeline_deleting(&world);
    let source = pipeline.add_source(Path::new("/card"), true).unwrap();
    // Allowed when the folder was added, so only the settings change.
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/card/out".into()),
            delete_after_import: DeleteAfterImport::Immediately,
            ..Default::default()
        })
        .unwrap();

    pipeline.scan_now(&HashSet::from([Some(source.id)]));
    pipeline.run_until_idle();

    assert!(exists(&world, "/card/DCIM/IMG_1.CR3") && exists(&world, "/card/DCIM/IMG_2.JPG"));
    assert!(warnings(&world).iter().any(|m| m.contains("overlap")));
}

#[test]
fn only_a_job_that_ended_as_imported_or_duplicate_is_deleted_right_away() {
    use crate::ports::pipeline::PipelineStore;
    // (stage the job is in, original deleted)
    let cases = [
        (JobStage::Imported, true),
        (JobStage::Duplicate, true),
        (JobStage::Queued, false),
        (JobStage::Copying, false),
        (JobStage::Failed, false),
    ];
    for (stage, deleted) in cases {
        let world = world_with(fs(), DeleteAfterImport::Ask);
        let id = import(&world, "/card");
        world
            .pipeline
            .set_source_delete_originals(id, Some(DeleteAfterImport::Immediately))
            .unwrap();
        let job = world
            .pipeline
            .cleanup_candidates(id, None)
            .unwrap()
            .remove(1);
        world
            .pipeline
            .set_job_stage(job.id, stage, job.photo_id, None)
            .unwrap();

        service(&world).after_import(job.id);

        assert_eq!(
            !exists(&world, "/card/DCIM/IMG_2.JPG"),
            deleted,
            "{stage:?}"
        );
    }
}

#[test]
fn a_copy_is_made_durable_before_its_original_is_deleted() {
    let world = world_with(fs(), DeleteAfterImport::Immediately);

    import_deleting(&world, "/card");

    let synced = world.fs.synced();
    for copy in [
        "/lib/2026/2026-10-12/IMG_1.CR3",
        "/lib/2026/2026-10-12/IMG_1.JPG",
        "/lib/2026/2026-10-13/IMG_2.JPG",
    ] {
        assert!(synced.contains(&PathBuf::from(copy)), "{copy}");
    }
    assert!(!exists(&world, "/card/DCIM/IMG_2.JPG"));
}

#[test]
fn originals_stay_when_the_copy_cannot_be_made_durable() {
    for right_away in [false, true] {
        let world = world_with(
            fs(),
            if right_away {
                DeleteAfterImport::Immediately
            } else {
                DeleteAfterImport::Ask
            },
        );
        let id = if right_away {
            world.fs.fail_sync();
            import_deleting(&world, "/card")
        } else {
            let id = import(&world, "/card");
            world.fs.fail_sync();
            id
        };
        let svc = service(&world);
        if !right_away {
            let plan = svc.plan(id).unwrap();
            let status = svc.run_now(id, plan.through_job_id).unwrap();
            assert_eq!(status.kept.len(), 2);
        }

        assert!(
            exists(&world, "/card/DCIM/IMG_1.CR3") && exists(&world, "/card/DCIM/IMG_2.JPG"),
            "right away: {right_away}"
        );
    }
}

#[test]
fn a_folder_culled_in_place_never_deletes_right_away() {
    let world = world_with(fs(), DeleteAfterImport::Immediately);
    let pipeline = pipeline_deleting(&world);
    let source = pipeline.add_source(Path::new("/card"), true).unwrap();
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            open_folder: Some("/card".into()),
            delete_after_import: DeleteAfterImport::Immediately,
            ..Default::default()
        })
        .unwrap();

    pipeline.scan_now(&HashSet::from([Some(source.id)]));
    pipeline.run_until_idle();

    assert!(exists(&world, "/card/DCIM/IMG_1.CR3") && exists(&world, "/card/DCIM/IMG_2.JPG"));
}
