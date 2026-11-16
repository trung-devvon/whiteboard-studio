use super::*;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wb-project-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}

#[test]
fn create_save_open_roundtrip() {
    let root = tmp("roundtrip").join("bai.wbproj");
    let mut p = new_project();
    let dir = ProjectDir::create(&root, &p).unwrap();
    assert!(root.join(ASSETS_DIR).is_dir());
    assert!(matches!(
        ProjectDir::create(&root, &p),
        Err(ProjectError::AlreadyExists(_))
    ));
    p.scenes.push(new_scene(2));
    dir.save(&p).unwrap();
    let o = ProjectDir::open(&root).unwrap();
    assert_eq!(o.project, p);
    assert!(o.recovery.is_none());
    // mở bằng đường dẫn file cũng được
    assert_eq!(ProjectDir::open(root.join(PROJECT_FILE)).unwrap().project, p);
    // không để lại file tạm
    let leftovers = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn autosave_is_offered_as_recovery_until_saved() {
    let root = tmp("recovery");
    let saved = new_project();
    let dir = ProjectDir::create(&root, &saved).unwrap();
    let mut edited = saved.clone();
    edited.scenes.push(new_scene(2));
    dir.autosave(&edited).unwrap();
    // "app bị tắt" → mở lại
    let o = ProjectDir::open(&root).unwrap();
    assert_eq!(o.project, saved, "project.json không bị autosave đụng tới");
    assert_eq!(o.recovery.as_ref(), Some(&edited));
    // lưu → hết bản khôi phục
    o.dir.save(&edited).unwrap();
    assert!(ProjectDir::open(&root).unwrap().recovery.is_none());
    assert!(!root.join(AUTOSAVE_FILE).exists());
}

#[test]
fn sample_project_parses() {
    let p = sample_project();
    assert_eq!(p.scenes.len(), 2);
    assert_eq!(p.settings.hand.as_deref(), Some(DEFAULT_HAND));
}

#[test]
fn broken_autosave_does_not_block_opening() {
    let root = tmp("broken");
    let dir = ProjectDir::create(&root, &new_project()).unwrap();
    fs::write(root.join(AUTOSAVE_FILE), "{ dở dang").unwrap();
    let o = ProjectDir::open(dir.root()).unwrap();
    assert!(o.recovery.is_none());
}

#[test]
fn newer_schema_is_rejected_with_clear_error() {
    let root = tmp("newer");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join(PROJECT_FILE), r#"{"schema_version": 999, "scenes": []}"#).unwrap();
    assert!(matches!(ProjectDir::open(&root), Err(ProjectError::TooNew(999))));
    fs::write(root.join(PROJECT_FILE), r#"{"scenes": []}"#).unwrap();
    assert!(matches!(ProjectDir::open(&root), Err(ProjectError::NoVersion)));
}

#[test]
fn import_asset_dedupes_by_content() {
    let root = tmp("assets");
    let dir = ProjectDir::create(&root, &new_project()).unwrap();
    let src_dir = tmp("assets-src");
    fs::create_dir_all(&src_dir).unwrap();
    let a = src_dir.join("Ảnh mèo.PNG");
    fs::write(&a, b"abc").unwrap();
    let r1 = dir.import_asset(&a).unwrap();
    let r2 = dir.import_asset(&a).unwrap();
    assert_eq!(r1, r2);
    assert!(r1.starts_with("assets/") && r1.ends_with(".png"), "{r1}");
    assert_eq!(fs::read(root.join(&r1)).unwrap(), b"abc");
    // cùng tên, khác nội dung → file khác
    fs::write(&a, b"xyz").unwrap();
    assert_ne!(dir.import_asset(&a).unwrap(), r1);
}
