//! The species chosen with Bird ID, stored in the SQLite catalog.

use std::path::PathBuf;

use picturesque_app::ports::catalog::PhotoCatalog;
use picturesque_core::birds::Species;
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_infra::catalog::SqliteCatalog;

fn insert(c: &SqliteCatalog, n: u8) -> PhotoId {
    c.insert_photo(&NewPhoto {
        metadata: Default::default(),
        assets: PhotoAssets::new(vec![Asset {
            format: MediaFormat::Jpeg,
            path: PathBuf::from(format!("{n}.JPG")),
            size: u64::from(n),
            hash: ContentHash::from_bytes([n; 32]),
        }])
        .unwrap(),
    })
    .unwrap()
}

fn species(common: &str, code: &str) -> Species {
    Species {
        common: common.into(),
        scientific: format!("{common} (scientific)"),
        code: code.into(),
    }
}

#[test]
fn keeps_replaces_and_forgets_a_photos_species() {
    let c = SqliteCatalog::open_in_memory().unwrap();
    let (a, b) = (insert(&c, 1), insert(&c, 2));
    let robin = species("American Robin", "amerob");
    let jay = species("Steller's Jay", "stejay");

    c.set_bird_species(a, std::slice::from_ref(&robin)).unwrap();
    c.set_bird_species(b, std::slice::from_ref(&robin)).unwrap();
    // Several species, once each, in the order given; replacing the old.
    c.set_bird_species(b, &[jay.clone(), robin.clone(), jay.clone()])
        .unwrap();

    assert_eq!(
        c.bird_species(&[a, b, PhotoId(99)]).unwrap(),
        [
            (a, vec![robin.clone()]),
            (b, vec![jay.clone(), robin.clone()])
        ]
    );

    c.set_bird_species(b, &[]).unwrap();
    assert_eq!(c.bird_species(&[a, b]).unwrap(), [(a, vec![robin])]);

    // It leaves with the photo.
    c.remove_photo(a).unwrap();
    assert!(c.bird_species(&[a]).unwrap().is_empty());
}
