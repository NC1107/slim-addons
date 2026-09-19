//! Word Guess: guess a five-letter word in six tries, typing each guess into
//! the scene.
//!
//! This is the module the `input` op made possible. A scene could be answered
//! by tapping and nothing else, so a guessing game meant an on-screen keyboard
//! of thirty tappable rects, which is a lot of ops to say "type a word".
//!
//! Every frame is a fresh sandboxed call. The whole game rides in the scene's
//! opaque `state` as `seed|guess,guess,...`, and nothing is remembered between
//! calls.
//!
//! The answer is not in the state. Only a seed is, and the answer is an index
//! derived from it - the same trick minesweeper uses for its mines, and for the
//! same reason: state rides the wire on every frame, so anything in it is
//! readable by whoever is playing. This is not airtight, since somebody who
//! runs the module themselves can derive the answer from the seed too. It is a
//! casual game and that is a fair trade; a module that must keep a secret from
//! the person playing needs host storage, which does not exist yet.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::words::WORDS;

const LENGTH: usize = 5;
const TRIES: usize = 6;

/// What a letter turned out to be, and the colour it draws as.
///
/// Green and amber are hex rather than theme tokens because the palette has no
/// pair for "right" and "nearly", and a guessing game where those two are not
/// instantly distinguishable is not playable. Everything else in the scene
/// stays on tokens.
const RIGHT: &str = "#4f9d69";
const MOVED: &str = "#e3b341";
const ABSENT: &str = "sunken";

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "wordguess" => Ok(play(input)),
        other => Err(format!("unknown command: {other}")),
    }
}

fn play(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return render(&Game::new(0));
    }
    let (action, mut game) = match serde_json::from_str::<Action>(trimmed) {
        Ok(a) => (a.action, Game::parse(&a.state)),
        Err(_) => (String::new(), Game::new(0)),
    };

    if let Some(guess) = action.strip_prefix("guess:") {
        game.guess(guess);
    } else if action == "reset" || action == "clear" || action == "new" {
        game = Game::new(game.seed.wrapping_add(1));
    }
    render(&game)
}

#[derive(Deserialize)]
struct Action {
    action: String,
    #[serde(default)]
    state: String,
}

struct Game {
    seed: u64,
    guesses: Vec<String>,
    /// Set when a guess could not be used, so the person is told why rather
    /// than watching their typing vanish.
    refused: Option<String>,
}

impl Game {
    fn new(seed: u64) -> Self {
        Game { seed, guesses: Vec::new(), refused: None }
    }

    fn parse(state: &str) -> Self {
        let (seed, rest) = state.split_once('|').unwrap_or((state, ""));
        let seed = seed.trim().parse::<u64>().unwrap_or(0);
        let guesses = rest
            .split(',')
            .map(str::trim)
            .filter(|g| g.len() == LENGTH && g.chars().all(|c| c.is_ascii_lowercase()))
            .take(TRIES)
            .map(str::to_owned)
            .collect();
        Game { seed, guesses, refused: None }
    }

    fn state(&self) -> String {
        format!("{}|{}", self.seed, self.guesses.join(","))
    }

    /// The answer, derived from the seed rather than stored beside it.
    fn answer(&self) -> &'static str {
        // A multiply-xor-shift, so neighbouring seeds do not give neighbouring
        // words; a plain modulo would walk the list one entry at a time.
        let mixed = (self.seed.wrapping_mul(6_364_136_223_846_793_005)) ^ 0x9E37_79B9_7F4A_7C15;
        WORDS[(mixed >> 17) as usize % WORDS.len()]
    }

    fn solved(&self) -> bool {
        self.guesses.last().map(|g| g == self.answer()).unwrap_or(false)
    }

    fn over(&self) -> bool {
        self.solved() || self.guesses.len() >= TRIES
    }

    fn guess(&mut self, raw: &str) {
        if self.over() {
            return;
        }
        let word: String = raw.trim().to_lowercase();
        if word.chars().count() != LENGTH || !word.chars().all(|c| c.is_ascii_lowercase()) {
            self.refused = Some(format!("{LENGTH} letters, a to z"));
            return;
        }
        if self.guesses.iter().any(|g| *g == word) {
            self.refused = Some("already guessed".to_owned());
            return;
        }
        self.guesses.push(word);
    }
}

/// Which colour each letter of `guess` earns against `answer`.
///
/// Two passes, because one is wrong in a way that shows: a letter is only
/// "moved" if a copy of it is still unaccounted for after every exact match has
/// claimed its own. Marking greedily left to right reports two ambers for a
/// doubled letter that appears once.
fn score(guess: &str, answer: &str) -> Vec<&'static str> {
    let guess: Vec<char> = guess.chars().collect();
    let answer: Vec<char> = answer.chars().collect();
    let mut marks = vec![ABSENT; guess.len()];
    let mut claimed = vec![false; answer.len()];

    for i in 0..guess.len() {
        if answer.get(i) == Some(&guess[i]) {
            marks[i] = RIGHT;
            claimed[i] = true;
        }
    }
    for i in 0..guess.len() {
        if marks[i] == RIGHT {
            continue;
        }
        if let Some(j) = (0..answer.len())
            .find(|&j| !claimed[j] && answer[j] == guess[i])
        {
            marks[i] = MOVED;
            claimed[j] = true;
        }
    }
    marks
}

/// One row of the board: a tile and its letter per column.
fn row(ops: &mut Vec<Value>, index: usize, guess: Option<&str>, marks: &[&str]) {
    let y = 4.0 + (index as f64) * 12.0;
    for col in 0..LENGTH {
        let x = 21.0 + (col as f64) * 12.0;
        ops.push(json!({
            "op": "rect",
            "x": x, "y": y, "w": 10.0, "h": 10.0, "r": 1.5,
            "fill": marks.get(col).copied().unwrap_or(ABSENT),
            "stroke": "border", "sw": 0.3
        }));
        if let Some(letter) = guess.and_then(|g| g.chars().nth(col)) {
            ops.push(json!({
                "op": "text",
                "x": x + 5.0, "y": y + 5.5,
                "s": letter.to_uppercase().to_string(),
                "fill": "text", "size": 6.0, "align": "center"
            }));
        }
    }
}

fn render(game: &Game) -> String {
    let answer = game.answer();
    let mut ops = Vec::new();
    for i in 0..TRIES {
        match game.guesses.get(i) {
            Some(guess) => row(&mut ops, i, Some(guess), &score(guess, answer)),
            None => row(&mut ops, i, None, &[]),
        }
    }

    if !game.over() {
        ops.push(json!({
            "op": "input",
            "x": 21.0, "y": 80.0, "w": 58.0,
            "submit": "guess",
            "placeholder": "type a word",
            "max": LENGTH
        }));
    }

    let status = if game.solved() {
        format!("got it in {}", game.guesses.len())
    } else if game.over() {
        format!("out of tries - it was {}", answer.to_uppercase())
    } else if let Some(why) = &game.refused {
        why.clone()
    } else {
        format!("{} of {} tries used", game.guesses.len(), TRIES)
    };

    json!({
        "$slim": "scene/1",
        "width": 100,
        "height": 100,
        "background": "surface",
        "ops": ops,
        "state": game.state(),
        "controls": ["reset"],
        "status": status,
    })
    .to_string()
}
