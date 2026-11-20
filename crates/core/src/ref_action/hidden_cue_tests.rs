use super::narration;
use crate::{RefEntry, SnapshotSurface};

fn entry(name: Option<&str>, surface: SnapshotSurface) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("menuitem", name);
    entry.source.source_surface = surface;
    entry
}

#[test]
fn menu_items_say_what_is_being_chosen() {
    assert_eq!(
        narration(&entry(Some("New"), SnapshotSurface::Menubar)),
        "Choosing New"
    );
    assert_eq!(
        narration(&entry(Some("Bold"), SnapshotSurface::Menu)),
        "Choosing Bold"
    );
}

#[test]
fn other_targets_use_their_name_or_a_calm_default() {
    assert_eq!(
        narration(&entry(Some(" Row 812 "), SnapshotSurface::Window)),
        "Row 812"
    );
    assert_eq!(
        narration(&entry(None, SnapshotSurface::Window)),
        "Working here"
    );
}
