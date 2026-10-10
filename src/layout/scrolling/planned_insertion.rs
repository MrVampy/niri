use niri_config::window_rule::{InsertionReference, TiledInsertion};

use super::{ColumnWidth, LayoutElement, ScrollingSpace, Tile};

impl<W: LayoutElement> ScrollingSpace<W> {
    fn insertion_neighbour(&self, references: &[InsertionReference]) -> Option<(usize, usize)> {
        references.iter().find_map(|reference| {
            let mut matching = self
                .columns
                .iter()
                .enumerate()
                .flat_map(|(column, windows)| {
                    windows
                        .tiles
                        .iter()
                        .enumerate()
                        .filter_map(move |(row, tile)| {
                            tile.window()
                                .matches_insertion_reference(reference)
                                .then_some((column, row))
                        })
                });
            let found = matching.next()?;
            matching.next().is_none().then_some(found)
        })
    }

    pub(in crate::layout) fn add_planned_tile(
        &mut self,
        insertion: &TiledInsertion,
        tile: Tile<W>,
        activate: bool,
        width: ColumnWidth,
        is_full_width: bool,
    ) {
        if let Some((column, row)) = self.insertion_neighbour(&insertion.above) {
            self.add_tile_to_column(column, Some(row + 1), tile, activate);
        } else if let Some((column, row)) = self.insertion_neighbour(&insertion.below) {
            self.add_tile_to_column(column, Some(row), tile, activate);
        } else {
            let column = self
                .insertion_neighbour(&insertion.left)
                .map(|(column, _)| column + 1)
                .or_else(|| {
                    self.insertion_neighbour(&insertion.right)
                        .map(|(column, _)| column)
                })
                .unwrap_or(0);
            self.add_tile(Some(column), tile, activate, width, is_full_width, None);
        }
    }
}
