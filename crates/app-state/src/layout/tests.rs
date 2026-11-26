//! Tests of the layout.

use super::testing::{folder, id, layout_with, top};
use super::*;

#[test]
fn showing_a_loose_terminal_of_the_same_folder_takes_the_focused_pane() {
  let mut layout = layout_with(&["t1"], vec![folder(1, false, &["t1", "t2"])]);
  layout.show("t2");
  assert_eq!(layout.shown(), vec!["t2"]);
  assert_eq!(layout.slots(), vec![folder(1, false, &["t1", "t2"])]);
}

#[test]
fn showing_a_loose_terminal_with_both_at_the_top_level_takes_the_focused_pane() {
  let mut layout = layout_with(&["t1"], vec![top("t1")]);
  layout.show("t2");
  assert_eq!(layout.shown(), vec!["t2"]);
}

#[test]
fn showing_a_loose_terminal_from_another_place_opens_its_own_tab() {
  for order in [
    vec![folder(1, false, &["t1"]), folder(2, false, &["t2"])],
    vec![folder(1, false, &["t1"]), top("t2")],
    vec![folder(1, false, &["t1"])],
    vec![top("t1"), folder(1, false, &["t2"])],
  ] {
    let mut layout = layout_with(&["t1"], order.clone());
    layout.show("t2");
    assert_eq!(layout.shown(), vec!["t1", "t2"], "with {order:?}");
    assert_eq!(layout.active_tab, 1);
    assert_eq!(layout.slots(), order);
  }
}

#[test]
fn showing_with_no_tab_opens_one() {
  let mut layout = layout_with(&[], vec![folder(1, false, &["t1"])]);
  layout.show("t1");
  assert_eq!(layout.shown(), vec!["t1"]);
}

#[test]
fn a_new_tab_goes_last_in_the_active_tabs_folder_and_opens_it() {
  let mut layout = layout_with(&["t1"], vec![folder(1, true, &["t1", "t2"]), top("t3")]);
  layout.new_tab_in_active_folder("t4");
  assert_eq!(layout.active_tab, 1);
  assert_eq!(
    layout.slots(),
    vec![folder(1, false, &["t1", "t2", "t4"]), top("t3")]
  );
}

#[test]
fn a_new_tab_from_the_top_level_or_from_no_tab_stays_unlisted() {
  let mut layout = layout_with(&["t1"], vec![top("t1"), folder(1, false, &[])]);
  layout.new_tab_in_active_folder("t2");
  assert_eq!(layout.slots(), vec![top("t1"), folder(1, false, &[])]);
  let mut empty = layout_with(&[], vec![folder(1, false, &[])]);
  empty.new_tab_in_active_folder("t1");
  assert_eq!(empty.shown(), vec!["t1"]);
  assert_eq!(empty.slots(), vec![folder(1, false, &[])]);
}

#[test]
fn forgetting_a_terminal_drops_its_place_and_name_and_keeps_its_folder() {
  let mut layout = layout_with(&[], vec![top("t1"), folder(1, false, &["t2"])]);
  layout.names.insert("t2".into(), "logs".into());
  layout.forget("t2");
  assert_eq!(layout.slots(), vec![top("t1"), folder(1, false, &[])]);
  assert!(layout.names.is_empty());
}

#[test]
fn stopping_showing_a_terminal_keeps_its_place_and_name() {
  let mut layout = layout_with(&["t1", "t2"], vec![folder(1, false, &["t1"]), top("t2")]);
  layout.names.insert("t1".into(), "server".into());
  layout.stop_showing("t1");
  assert_eq!(layout.shown(), vec!["t2"]);
  assert_eq!(layout.active_tab, 0);
  assert_eq!(layout.slots(), vec![folder(1, false, &["t1"]), top("t2")]);
  assert_eq!(layout.names.get("t1").map(String::as_str), Some("server"));
}

#[test]
fn keeping_only_live_terminals_keeps_the_others_places() {
  let mut layout = layout_with(&["t1", "t2"], vec![top("t1"), top("t2")]);
  layout.keep_only(&["t2"]);
  assert_eq!(layout.shown(), vec!["t2"]);
  assert_eq!(layout.slots(), vec![top("t1"), top("t2")]);
}

#[test]
fn a_split_lists_its_terminal_after_the_tabs_last_listed_one() {
  let mut layout = layout_with(&["t1"], vec![folder(1, false, &["t1", "t2"]), top("t9")]);
  layout.split(Axis::Horizontal, "t3");
  assert_eq!(
    layout.slots(),
    vec![folder(1, false, &["t1", "t3", "t2"]), top("t9")]
  );
  layout.focus("t1");
  layout.split(Axis::Vertical, "t4");
  assert_eq!(
    layout.slots(),
    vec![folder(1, false, &["t1", "t3", "t4", "t2"]), top("t9")]
  );
}

#[test]
fn a_split_at_the_top_level_lists_its_terminal_there() {
  let mut layout = layout_with(&["t1"], vec![top("t1"), top("t2")]);
  layout.split(Axis::Horizontal, "t3");
  assert_eq!(layout.slots(), vec![top("t1"), top("t3"), top("t2")]);
}

#[test]
fn a_split_of_an_unlisted_tab_lists_nothing() {
  let mut layout = layout_with(&["t1"], vec![top("t2")]);
  layout.split(Axis::Horizontal, "t3");
  assert_eq!(layout.slots(), vec![top("t2")]);
}

#[test]
fn reads_a_layout_saved_before_folders() {
  let json = r#"{"tabs":[{"root":{"leaf":{"terminal":"t1"}},"focused":"t1"}],
  "active_tab":0,"order":["t1","t2"],"names":{"t2":"logs"}}"#;
  let layout: Layout = serde_json::from_str(json).expect("a 0.1.1 layout reads");
  assert_eq!(layout.slots(), vec![top("t1"), top("t2")]);
  assert_eq!(layout.shown(), vec!["t1"]);
  assert_eq!(layout.names.get("t2").map(String::as_str), Some("logs"));
}

#[test]
fn folders_are_saved_beside_a_flat_order_and_read_back() {
  let layout = layout_with(&[], vec![top("t1"), folder(1, true, &["t2"]), top("t3")]);
  let saved = serde_json::to_value(&layout).expect("a layout serializes");
  assert_eq!(saved["order"], serde_json::json!(["t1", "t2", "t3"]));
  assert_eq!(
    saved["folders"],
    serde_json::json!([
      {"id": 1, "name": "f1", "collapsed": true, "position": 1, "terminals": ["t2"]}
    ])
  );
  let read: Layout = serde_json::from_value(saved).expect("it reads back");
  assert_eq!(read, layout);
}

/// The shape 0.1.1 reads: no folders, `order` is a list of addresses.
#[derive(Deserialize)]
struct Released {
  tabs: Vec<Tab>,
  active_tab: usize,
  #[serde(default)]
  order: Vec<String>,
  #[serde(default)]
  names: BTreeMap<String, String>,
}

#[test]
fn a_layout_with_folders_still_reads_in_the_released_shape() {
  let mut layout = layout_with(
    &["t1"],
    vec![top("t1"), folder(1, false, &["t3", "t4"]), top("t2")],
  );
  layout.names.insert("t2".into(), "logs".into());
  let saved = serde_json::to_string(&layout).expect("a layout serializes");
  let old: Released = serde_json::from_str(&saved).expect("0.1.1 reads it");
  assert_eq!(old.tabs.len(), 1);
  assert_eq!(old.active_tab, 0);
  assert_eq!(old.order, vec!["t1", "t3", "t4", "t2"]);
  assert_eq!(old.names.get("t2").map(String::as_str), Some("logs"));
}

#[test]
fn a_folder_without_its_optional_fields_reads() {
  let json = r#"{"tabs":[],"active_tab":0,"folders":[{"id":2,"name":"api"}]}"#;
  let layout: Layout = serde_json::from_str(json).expect("a short folder reads");
  assert_eq!(
    layout.slots(),
    vec![Slot::Folder {
      id: id(2),
      name: "api".into(),
      collapsed: false,
      terminals: Vec::new(),
    }]
  );
}

#[test]
fn places_heal_a_layout_whose_views_disagree() {
  let json = r#"{"tabs":[],"active_tab":0,"order":["t1","t1","t2"],
  "folders":[{"id":1,"name":"a","position":9,"terminals":["t3","t2"]},
  {"id":1,"name":"dup","terminals":["t1"]}]}"#;
  let layout: Layout = serde_json::from_str(json).expect("it reads");
  assert_eq!(
    layout.slots(),
    vec![
      top("t1"),
      top("t2"),
      Slot::Folder {
        id: id(1),
        name: "a".into(),
        collapsed: false,
        terminals: vec!["t3".into()],
      },
    ]
  );
}

#[test]
fn places_read_depth_first_and_skip_a_terminal_listed_again() {
  let layout = layout_with(
    &[],
    vec![top("t1"), folder(1, false, &["t2", "t1"]), top("t3")],
  );
  let listed = Folder {
    id: id(1),
    name: "f1".into(),
    collapsed: false,
    position: 1,
    terminals: vec!["t2".into(), "t1".into()],
  };
  assert_eq!(
    layout.places(),
    vec![
      Place::Terminal {
        address: "t1",
        folder: None
      },
      Place::Folder(&listed),
      Place::Terminal {
        address: "t2",
        folder: Some(id(1))
      },
      Place::Terminal {
        address: "t3",
        folder: None
      },
    ]
  );
}

#[test]
fn a_new_folder_id_is_one_more_than_the_largest() {
  assert_eq!(layout_with(&[], Vec::new()).next_folder_id(), id(1));
  let layout = layout_with(
    &[],
    vec![folder(4, false, &[]), top("t1"), folder(2, false, &[])],
  );
  assert_eq!(layout.next_folder_id(), id(5));
}

#[test]
fn renaming_a_folder_trims_and_a_blank_name_keeps_the_old_one() {
  let mut layout = layout_with(&[], vec![folder(1, false, &[])]);
  layout.rename_folder(id(1), "  api  ");
  layout.rename_folder(id(1), "   ");
  assert_eq!(
    layout.folder(id(1)).map(|folder| folder.name.as_str()),
    Some("api")
  );
}

#[test]
fn toggling_a_folder_collapses_then_opens_it() {
  let mut layout = layout_with(&[], vec![folder(1, false, &[])]);
  layout.toggle_folder(id(1));
  assert_eq!(layout.slots(), vec![folder(1, true, &[])]);
  layout.toggle_folder(id(1));
  assert_eq!(layout.slots(), vec![folder(1, false, &[])]);
}

#[test]
fn deleting_a_folder_leaves_its_terminals_where_it_was() {
  let mut layout = layout_with(
    &[],
    vec![top("t1"), folder(1, true, &["t2", "t3"]), top("t4")],
  );
  layout.delete_folder(id(1));
  assert_eq!(
    layout.slots(),
    vec![top("t1"), top("t2"), top("t3"), top("t4")]
  );
}
