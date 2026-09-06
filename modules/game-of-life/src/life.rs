//! Conway's Game of Life on a fixed toroidal board.
//!
//! The board is a fixed [`COLS`] x [`ROWS`] grid that wraps at every edge, so
//! a glider sails off one side and reappears on the other and the world never
//! runs out of room. B3/S23 rules. Nothing here draws or knows about JSON;
//! `scene.rs` turns a [`Board`] into something the client can paint, and
//! `lib.rs` wires the two together.

use crate::rng::SplitMix64;

pub const COLS: usize = 48;
pub const ROWS: usize = 48;

/// A single generation: one `bool` per cell, row-major, always `COLS * ROWS`
/// long. Carries its own generation counter so the caption can show it.
pub struct Board {
    pub gen: u64,
    cells: Vec<bool>,
}

/// A pattern at its own natural size, before it is centered onto a [`Board`].
struct Pattern {
    w: usize,
    h: usize,
    live: Vec<(usize, usize)>,
}

impl Board {
    fn empty() -> Self {
        Board {
            gen: 0,
            cells: vec![false; COLS * ROWS],
        }
    }

    /// An all-dead board at generation zero: the `clear` control, a blank
    /// canvas to draw your own pattern onto.
    pub fn blank() -> Self {
        Board::empty()
    }

    /// The initial board for a fresh run: a named preset, an ASCII drawing,
    /// `random` for a soup, or - for empty input - a Gosper glider gun, which
    /// never settles and so is the liveliest possible first impression.
    pub fn seed(input: &str) -> Self {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Board::from_pattern(&gosper_gun());
        }
        if trimmed.eq_ignore_ascii_case("random") {
            return Board::random(input.as_bytes());
        }
        if let Some(pattern) = preset(trimmed) {
            return Board::from_pattern(&pattern);
        }
        Board::from_pattern(&parse_ascii(trimmed))
    }

    /// A soup filling roughly a third of the board, seeded from `salt` so the
    /// same salt reproduces the same soup and successive `random` presses (which
    /// pass the changing generation as salt) differ.
    pub fn random(salt: &[u8]) -> Self {
        let mut rng = SplitMix64::from_bytes(salt);
        let mut board = Board::empty();
        for cell in board.cells.iter_mut() {
            *cell = rng.next_u64() % 100 < 32;
        }
        board
    }

    fn from_pattern(pattern: &Pattern) -> Self {
        let mut board = Board::empty();
        let offset_col = COLS.saturating_sub(pattern.w) / 2;
        let offset_row = ROWS.saturating_sub(pattern.h) / 2;
        for &(col, row) in &pattern.live {
            let c = offset_col + col;
            let r = offset_row + row;
            if c < COLS && r < ROWS {
                board.cells[r * COLS + c] = true;
            }
        }
        board
    }

    fn get(&self, col: usize, row: usize) -> bool {
        self.cells[row * COLS + col]
    }

    /// The next generation under B3/S23, with every edge wrapping to the far
    /// side. Returns the new board and whether anything at all changed, so the
    /// caller can stop animating a board that has settled.
    pub fn step(&self) -> (Board, bool) {
        let mut next = Board {
            gen: self.gen + 1,
            cells: vec![false; COLS * ROWS],
        };
        let mut changed = false;
        for row in 0..ROWS {
            for col in 0..COLS {
                let neighbors = self.neighbors(col, row);
                let alive = self.get(col, row);
                let next_alive = matches!((alive, neighbors), (true, 2) | (_, 3));
                next.cells[row * COLS + col] = next_alive;
                if next_alive != alive {
                    changed = true;
                }
            }
        }
        (next, changed)
    }

    fn neighbors(&self, col: usize, row: usize) -> u8 {
        let mut count = 0u8;
        for dr in [ROWS - 1, 0, 1] {
            for dc in [COLS - 1, 0, 1] {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let r = (row + dr) % ROWS;
                let c = (col + dc) % COLS;
                if self.cells[r * COLS + c] {
                    count += 1;
                }
            }
        }
        count
    }

    /// Flips one cell, leaving the generation untouched: this is a person
    /// drawing on the board between steps, not the simulation advancing.
    pub fn toggle(&mut self, col: usize, row: usize) {
        if col < COLS && row < ROWS {
            let idx = row * COLS + col;
            self.cells[idx] = !self.cells[idx];
        }
    }

    pub fn population(&self) -> usize {
        self.cells.iter().filter(|&&c| c).count()
    }

    /// The client-facing cell string: one `'0'` or `'1'` per cell, row-major.
    pub fn cell_string(&self) -> String {
        self.cells
            .iter()
            .map(|&c| if c { '1' } else { '0' })
            .collect()
    }

    /// Packs the whole board into one opaque line the client echoes back
    /// verbatim on the next step: `cols,rows,gen,<cell string>`.
    pub fn to_state(&self) -> String {
        format!("{},{},{},{}", COLS, ROWS, self.gen, self.cell_string())
    }

    /// Rebuilds a board from a [`to_state`] string, falling back to an empty
    /// board on anything malformed rather than trapping.
    pub fn from_state(state: &str) -> Self {
        let mut parts = state.splitn(4, ',');
        let cols = parts.next().and_then(|p| p.parse::<usize>().ok());
        let _rows = parts.next().and_then(|p| p.parse::<usize>().ok());
        let gen = parts.next().and_then(|p| p.parse::<u64>().ok());
        let body = parts.next();
        match (cols, gen, body) {
            (Some(cols), Some(gen), Some(body)) if cols == COLS && body.len() == COLS * ROWS => {
                Board {
                    gen,
                    cells: body.bytes().map(|b| b == b'1').collect(),
                }
            }
            _ => Board::empty(),
        }
    }
}

fn parse_ascii(text: &str) -> Pattern {
    let rows: Vec<&str> = text.lines().collect();
    let mut live = Vec::new();
    let mut width = 0;
    for (row, line) in rows.iter().enumerate() {
        let mut col = 0;
        for ch in line.chars() {
            if matches!(ch, '#' | 'O' | 'o' | '*' | 'X' | 'x' | '@' | '1') {
                live.push((col, row));
            }
            col += 1;
        }
        width = width.max(col);
    }
    Pattern {
        w: width,
        h: rows.len(),
        live,
    }
}

fn preset(name: &str) -> Option<Pattern> {
    let art = match name.to_ascii_lowercase().as_str() {
        "glider" => ".#.\n..#\n###",
        "blinker" => "###",
        "toad" => ".###\n###.",
        "beacon" => "##..\n##..\n..##\n..##",
        "pulsar" => PULSAR,
        "gun" | "glider-gun" | "glidergun" => return Some(gosper_gun()),
        "acorn" => ".#.....\n...#...\n##..###",
        "r-pentomino" | "rpentomino" | "pentomino" => ".##\n##.\n.#.",
        "lwss" | "spaceship" => ".#..#\n#....\n#...#\n####.",
        _ => return None,
    };
    Some(parse_ascii(art))
}

const PULSAR: &str = "..###...###..\n.............\n#....#.#....#\n#....#.#....#\n#....#.#....#\n..###...###..\n.............\n..###...###..\n#....#.#....#\n#....#.#....#\n#....#.#....#\n.............\n..###...###..";

fn gosper_gun() -> Pattern {
    let coords: &[(usize, usize)] = &[
        (24, 0),
        (22, 1),
        (24, 1),
        (12, 2),
        (13, 2),
        (20, 2),
        (21, 2),
        (34, 2),
        (35, 2),
        (11, 3),
        (15, 3),
        (20, 3),
        (21, 3),
        (34, 3),
        (35, 3),
        (0, 4),
        (1, 4),
        (10, 4),
        (16, 4),
        (20, 4),
        (21, 4),
        (0, 5),
        (1, 5),
        (10, 5),
        (14, 5),
        (16, 5),
        (17, 5),
        (22, 5),
        (24, 5),
        (10, 6),
        (16, 6),
        (24, 6),
        (11, 7),
        (15, 7),
        (12, 8),
        (13, 8),
    ];
    Pattern {
        w: 36,
        h: 9,
        live: coords.to_vec(),
    }
}
