use niri_config::window_rule::{InsertionReference, TiledInsertion};

use super::*;

fn permutations(values: &[usize]) -> Vec<Vec<usize>> {
    if values.is_empty() {
        return vec![vec![]];
    }
    let mut result = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let mut remaining = values.to_vec();
        remaining.remove(index);
        for mut order in permutations(&remaining) {
            order.insert(0, *value);
            result.push(order);
        }
    }
    result
}

fn references(ids: impl Iterator<Item = usize>) -> Vec<InsertionReference> {
    ids.map(|id| InsertionReference {
        id: Some(id as u64),
        ..Default::default()
    })
    .collect()
}

fn add(layout: &mut Layout<TestWindow>, id: usize, columns: &[Vec<usize>]) {
    let (column, row) = columns
        .iter()
        .enumerate()
        .find_map(|(column, members)| {
            members
                .iter()
                .position(|member| *member == id)
                .map(|row| (column, row))
        })
        .unwrap();
    let insertion = TiledInsertion {
        left: references(
            columns[..column]
                .iter()
                .rev()
                .flat_map(|members| members.iter().copied()),
        ),
        right: references(
            columns[column + 1..]
                .iter()
                .flat_map(|members| members.iter().copied()),
        ),
        above: references(columns[column][..row].iter().rev().copied()),
        below: references(columns[column][row + 1..].iter().copied()),
    };
    let mut params = TestWindowParams::new(id);
    params.rules = Some(ResolvedWindowRules {
        open_tiled_insertion: Some(insertion),
        ..Default::default()
    });
    layout.add_window(
        TestWindow::new(params),
        AddWindowTarget::Auto,
        None,
        None,
        false,
        false,
        ActivateWindow::No,
    );
    layout.verify_invariants();
}

fn positions(layout: &Layout<TestWindow>) -> Vec<(usize, (usize, usize))> {
    layout
        .active_workspace()
        .unwrap()
        .tiles_with_ipc_layouts()
        .map(|(tile, layout)| (*tile.window().id(), layout.pos_in_scrolling_layout.unwrap()))
        .collect()
}

#[test]
fn every_arrival_permutation_inserts_into_the_same_columns_without_focusing() {
    let columns = vec![vec![1], vec![2], vec![3], vec![4]];
    for order in permutations(&[1, 2, 3, 4]) {
        let mut layout = check_ops([Op::AddOutput(1)]);
        for id in order {
            let focused = layout
                .active_workspace()
                .unwrap()
                .active_window()
                .map(|window| *window.id());
            add(&mut layout, id, &columns);
            if let Some(focused) = focused {
                assert_eq!(
                    *layout
                        .active_workspace()
                        .unwrap()
                        .active_window()
                        .unwrap()
                        .id(),
                    focused
                );
            }
        }
        assert_eq!(
            positions(&layout),
            vec![(1, (1, 1)), (2, (2, 1)), (3, (3, 1)), (4, (4, 1))]
        );
    }
}

#[test]
fn absent_middle_members_are_skipped_and_late_arrivals_fill_the_gap() {
    let columns = vec![vec![1], vec![2], vec![3], vec![4]];
    let mut layout = check_ops([Op::AddOutput(1)]);
    add(&mut layout, 4, &columns);
    add(&mut layout, 1, &columns);
    assert_eq!(positions(&layout), vec![(1, (1, 1)), (4, (2, 1))]);
    add(&mut layout, 3, &columns);
    add(&mut layout, 2, &columns);
    assert_eq!(
        positions(&layout),
        vec![(1, (1, 1)), (2, (2, 1)), (3, (3, 1)), (4, (4, 1))]
    );
}

#[test]
fn fixed_windows_and_the_owners_rearranged_order_remain_in_place() {
    let columns = vec![vec![1], vec![2], vec![3], vec![4]];
    let mut layout = check_ops([
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(3),
        },
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
    ]);
    let focused = *layout
        .active_workspace()
        .unwrap()
        .active_window()
        .unwrap()
        .id();
    add(&mut layout, 2, &columns);
    add(&mut layout, 4, &columns);
    assert_eq!(
        positions(&layout),
        vec![(3, (1, 1)), (4, (2, 1)), (1, (3, 1)), (2, (4, 1))]
    );
    assert_eq!(
        *layout
            .active_workspace()
            .unwrap()
            .active_window()
            .unwrap()
            .id(),
        focused
    );
}

#[test]
fn every_stacked_arrival_permutation_inserts_above_or_below_present_members() {
    let columns = vec![vec![1, 2, 3], vec![4]];
    for order in permutations(&[1, 2, 3, 4]) {
        let mut layout = check_ops([Op::AddOutput(1)]);
        for id in order {
            add(&mut layout, id, &columns);
        }
        assert_eq!(
            positions(&layout),
            vec![(1, (1, 1)), (2, (1, 2)), (3, (1, 3)), (4, (2, 1))]
        );
    }
}
