use super::*;

fn ids(layout: &Layout<TestWindow>) -> Vec<usize> {
    layout.windows().map(|(_, window)| *window.id()).collect()
}

fn focused(layout: &Layout<TestWindow>) -> usize {
    *layout
        .active_workspace()
        .unwrap()
        .active_window()
        .unwrap()
        .id()
}

#[test]
fn targeted_column_moves_preserve_focus_across_index_shifts() {
    let mut layout = check_ops([
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::AddWindow {
            params: TestWindowParams::new(3),
        },
    ]);
    layout.move_column_to_index_by_id(&1, 3);
    layout.verify_invariants();
    assert_eq!(ids(&layout), [2, 3, 1]);
    assert_eq!(focused(&layout), 3);
    layout.move_column_to_index_by_id(&1, 1);
    layout.verify_invariants();
    assert_eq!(ids(&layout), [1, 2, 3]);
    assert_eq!(focused(&layout), 3);
    layout.move_column_to_index_by_id(&99, 1);
    assert_eq!(ids(&layout), [1, 2, 3]);
}

#[test]
fn targeted_join_and_row_move_leave_the_focused_window_selected() {
    let mut layout = check_ops([
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::AddWindow {
            params: TestWindowParams::new(3),
        },
    ]);
    layout.consume_or_expel_window_left(Some(&2));
    layout.move_window_up_by_id(&2);
    layout.verify_invariants();
    assert_eq!(ids(&layout), [2, 1, 3]);
    assert_eq!(focused(&layout), 3);
    layout.move_window_up_by_id(&2);
    layout.verify_invariants();
    assert_eq!(ids(&layout), [2, 1, 3]);
    layout.consume_or_expel_window_left(Some(&2));
    layout.move_column_to_index_by_id(&2, 3);
    layout.verify_invariants();
    assert_eq!(ids(&layout), [1, 3, 2]);
    assert_eq!(focused(&layout), 3);
}

#[test]
fn targeted_width_display_and_downward_moves_preserve_focus_and_report_display() {
    let mut layout = check_ops([
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::AddWindow {
            params: TestWindowParams::new(3),
        },
    ]);
    let other_width = layout
        .windows()
        .find(|(_, window)| *window.id() == 3)
        .unwrap()
        .1
        .requested_size();
    layout.consume_or_expel_window_left(Some(&2));
    layout.set_column_width_by_id(&2, SizeChange::SetFixed(600));
    layout.verify_invariants();
    assert_eq!(focused(&layout), 3);
    for (_, window) in layout.windows() {
        if *window.id() == 3 {
            assert_eq!(window.requested_size(), other_width);
        } else {
            assert_eq!(window.requested_size().unwrap().w, 600);
        }
    }
    layout.set_column_display_by_id(&2, niri_ipc::ColumnDisplay::Tabbed);
    layout.verify_invariants();
    let display = layout
        .active_workspace()
        .unwrap()
        .tiles_with_ipc_layouts()
        .find(|(tile, _)| *tile.window().id() == 1)
        .unwrap()
        .1
        .column_display;
    assert_eq!(display, Some(niri_ipc::ColumnDisplay::Tabbed));
    layout.move_window_down_by_id(&1);
    layout.verify_invariants();
    assert_eq!(ids(&layout), [2, 1, 3]);
    assert_eq!(focused(&layout), 3);
    layout.set_column_display_by_id(&1, niri_ipc::ColumnDisplay::Normal);
    let display = layout
        .active_workspace()
        .unwrap()
        .tiles_with_ipc_layouts()
        .find(|(tile, _)| *tile.window().id() == 1)
        .unwrap()
        .1
        .column_display;
    assert_eq!(display, Some(niri_ipc::ColumnDisplay::Normal));
    assert_eq!(focused(&layout), 3);
}

#[test]
fn column_width_settings_roundtrip_without_window_border_inference() {
    let mut layout = check_ops([
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::AddWindow {
            params: TestWindowParams::new(3),
        },
    ]);
    layout.consume_or_expel_window_left(Some(&2));
    for width in [
        niri_ipc::ColumnWidthSetting::Fixed(603.25),
        niri_ipc::ColumnWidthSetting::Proportion(0.6),
    ] {
        layout.set_column_width_setting_by_id(&2, width);
        layout.verify_invariants();
        for (tile, geometry) in layout.active_workspace().unwrap().tiles_with_ipc_layouts() {
            if *tile.window().id() != 3 {
                assert_eq!(geometry.column_width, Some(width));
            }
        }
        assert_eq!(focused(&layout), 3);
    }
    layout.set_column_width_setting_by_id(&2, niri_ipc::ColumnWidthSetting::Fixed(f64::NAN));
    layout.verify_invariants();
    assert_eq!(focused(&layout), 3);
}
