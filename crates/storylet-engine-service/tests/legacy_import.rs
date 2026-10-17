use storylet_engine_service::legacy_import::maybe_import_legacy_database;
use storylet_engine_service::store::Store;

#[test]
fn test_legacy_import_pipeline() {
    let mut store = Store::open_in_memory().unwrap();
    let path1 = if std::path::Path::new("data/storylet_studio.db").exists() { "data/storylet_studio.db" } else { "../../data/storylet_studio.db" };
    let path2 = if std::path::Path::new("data/storylet_engine.db").exists() { "data/storylet_engine.db" } else { "../../data/storylet_engine.db" };
    let res = maybe_import_legacy_database(&mut store, path1, path2);
    assert!(res.is_ok(), "Import failed: {:?}", res);

    let storylets = store.list_storylets().unwrap();
    assert_eq!(storylets.len(), 500, "Expected 500 storylets to be imported");

    // Verify user decision: Require fresh approval!
    for st in &storylets {
        assert_eq!(st.approved_revision, None, "Imported storylet {} must have approved_revision=None", st.id);
    }

    let qualities = store.list_qualities().unwrap();
    assert!(!qualities.is_empty(), "Expected qualities to be imported");

    let characters = store.list_characters().unwrap();
    assert!(!characters.is_empty(), "Expected characters to be imported");

    let locations = store.list_locations().unwrap();
    assert!(!locations.is_empty(), "Expected locations to be imported");

    let scenarios = store.list_scenarios().unwrap();
    assert!(!scenarios.is_empty(), "Expected scenarios to be imported");
}
