use emux_vt::Parser;
use emux_term::{Screen, grid::Cell};

pub struct TestTerminal {
    pub screen: Screen,
    pub parser: Parser,
}

impl TestTerminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            screen: Screen::new(cols, rows),
            parser: Parser::new(),
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.screen, bytes);
    }

    pub fn push_str(&mut self, s: &str) {
        self.push(s.as_bytes());
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.screen.cursor.row, self.screen.cursor.col)
    }

    pub fn cell(&self, row: usize, col: usize) -> &Cell {
        self.screen.grid.cell(row, col)
    }

    pub fn row_text(&self, row: usize) -> String {
        self.screen.grid.row_text(row)
    }

    pub fn assert_cursor(&self, row: usize, col: usize) {
        assert_eq!(self.cursor(), (row, col), "cursor at ({}, {})", row, col);
    }

    pub fn assert_cell_char(&self, row: usize, col: usize, expected: char) {
        assert_eq!(self.cell(row, col).c, expected, "cell ({},{}) char", row, col);
    }

    pub fn assert_cell_width(&self, row: usize, col: usize, expected: u8) {
        assert_eq!(self.cell(row, col).width, expected, "cell ({},{}) width", row, col);
    }

    pub fn assert_row_text(&self, row: usize, expected: &str) {
        let text = self.row_text(row);
        let trimmed = text.trim_end();
        assert_eq!(trimmed, expected, "row {} text", row);
    }
}
