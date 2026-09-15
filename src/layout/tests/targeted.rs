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
        layout.set_column_width_setting_by_id(&2, width, false);
        layout.verify_invariants();
        for (tile, geometry) in layout.active_workspace().unwrap().tiles_with_ipc_layouts() {
            if *tile.window().id() != 3 {
                assert_eq!(geometry.column_width, Some(width));
            }
        }
        assert_eq!(focused(&layout), 3);
    }
    layout.set_column_width_setting_by_id(&2, niri_ipc::ColumnWidthSetting::Proportion(0.6), true);
    let geometry = layout
        .active_workspace()
        .unwrap()
        .tiles_with_ipc_layouts()
        .find(|(tile, _)| *tile.window().id() == 2)
        .unwrap()
        .1;
    assert_eq!(
        geometry.column_width,
        Some(niri_ipc::ColumnWidthSetting::Proportion(0.6))
    );
    assert_eq!(geometry.column_full_width, Some(true));
    layout.set_maximized(&2, true);
    layout.set_column_width_setting_by_id(&2, niri_ipc::ColumnWidthSetting::Proportion(0.6), false);
    assert!(layout
        .windows()
        .find(|(_, window)| *window.id() == 2)
        .unwrap()
        .1
        .pending_sizing_mode()
        .is_maximized());
    layout.set_column_width_setting_by_id(&2, niri_ipc::ColumnWidthSetting::Fixed(f64::NAN), false);
    layout.verify_invariants();
    assert_eq!(focused(&layout), 3);
}

#[test]
fn floating_positions_use_the_same_origin_as_the_position_action() {
    let mut params = TestWindowParams::new(1);
    params.is_floating = true;
    let options = Options {
        layout: niri_config::Layout {
            struts: Struts {
                left: FloatOrInt(20.0),
                top: FloatOrInt(30.0),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let mut layout = check_ops_with_options(options, [Op::AddOutput(1), Op::AddWindow { params }]);
    layout.move_floating_window(
        Some(&1),
        niri_ipc::PositionChange::SetFixed(40.0),
        niri_ipc::PositionChange::SetFixed(70.0),
        false,
    );
    layout.verify_invariants();
    let geometry = layout
        .active_workspace()
        .unwrap()
        .tiles_with_ipc_layouts()
        .next()
        .unwrap()
        .1;
    assert_eq!(geometry.pos_in_working_area, Some((40.0, 70.0)));
    assert_ne!(
        geometry.pos_in_working_area,
        geometry.tile_pos_in_workspace_view
    );
}
