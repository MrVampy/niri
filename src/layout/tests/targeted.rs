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
