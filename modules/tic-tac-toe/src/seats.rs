//! Who holds which side of a board. A module keeps nothing between frames, so
//! the two caller ids ride in the scene state next to the board.
//!
//! The first person to make a legal move takes the first side; the first other
//! person to move after that takes the second. A third person only watches.
//! The state is echoed by the client, so this stops mistakes (a double tap, the
//! wrong person moving) but not a member who forges the state string.

/// Longest caller id accepted; the host sends a 64-character hex digest.
const MAX_ID: usize = 64;

/// Why a move was not made. The note is shown to everyone viewing the board.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Anonymous,
    NotYourTurn,
    Watching,
    Illegal(&'static str),
}

impl Refusal {
    pub fn note(&self) -> &'static str {
        match self {
            Refusal::Anonymous => "this server did not say who you are",
            Refusal::NotYourTurn => "not your turn",
            Refusal::Watching => "both sides are taken, you are watching",
            Refusal::Illegal(why) => why,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seats {
    first: String,
    second: String,
}

/// An id is lowercase hex, which also keeps it from breaking the `|` framing of the state.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn clean(id: &str) -> String {
    if valid_id(id) {
        id.to_string()
    } else {
        String::new()
    }
}

impl Seats {
    pub fn parse(first: &str, second: &str) -> Self {
        Seats {
            first: clean(first),
            second: clean(second),
        }
    }

    pub fn first(&self) -> &str {
        &self.first
    }

    pub fn second(&self) -> &str {
        &self.second
    }

    pub fn holds(&self, caller: &str) -> bool {
        valid_id(caller) && (caller == self.first || caller == self.second)
    }

    /// Whether `caller` may wipe the board: anyone once it is over or before
    /// the second side is taken, otherwise only a player.
    pub fn may_reset(&self, caller: &str, over: bool) -> bool {
        valid_id(caller) && (over || self.second.is_empty() || self.holds(caller))
    }

    /// The seats as they stand after `caller` moves, or why they may not.
    pub fn admit(&self, caller: &str, first_to_move: bool) -> Result<Seats, Refusal> {
        if !valid_id(caller) {
            return Err(Refusal::Anonymous);
        }
        let turn_ok = |mine_is_first: bool| {
            if mine_is_first == first_to_move {
                Ok(self.clone())
            } else {
                Err(Refusal::NotYourTurn)
            }
        };
        if caller == self.first {
            return turn_ok(true);
        }
        if caller == self.second {
            return turn_ok(false);
        }
        let mut next = self.clone();
        if self.first.is_empty() && first_to_move {
            next.first = caller.to_string();
            Ok(next)
        } else if self.second.is_empty() && !first_to_move && !self.first.is_empty() {
            next.second = caller.to_string();
            Ok(next)
        } else if self.second.is_empty() {
            Err(Refusal::NotYourTurn)
        } else {
            Err(Refusal::Watching)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aa";
    const B: &str = "bb";
    const C: &str = "cc";

    #[test]
    fn the_first_mover_takes_the_first_side() {
        let s = Seats::default().admit(A, true).unwrap();
        assert_eq!(s.first(), A);
    }

    #[test]
    fn the_same_person_cannot_take_both_sides() {
        let s = Seats::default().admit(A, true).unwrap();
        assert_eq!(s.admit(A, false), Err(Refusal::NotYourTurn));
    }

    #[test]
    fn a_second_person_takes_the_second_side_on_their_turn_only() {
        let s = Seats::default().admit(A, true).unwrap();
        assert_eq!(s.admit(B, true), Err(Refusal::NotYourTurn));
        assert_eq!(s.admit(B, false).unwrap().second(), B);
    }

    #[test]
    fn a_seated_player_cannot_move_twice() {
        let s = Seats::default().admit(A, true).unwrap();
        assert_eq!(s.admit(A, false), Err(Refusal::NotYourTurn));
    }

    #[test]
    fn a_third_person_only_watches() {
        let s = Seats::parse(A, B);
        assert_eq!(s.admit(C, true), Err(Refusal::Watching));
        assert_eq!(s.admit(C, false), Err(Refusal::Watching));
    }

    #[test]
    fn an_unidentified_caller_never_moves() {
        assert_eq!(Seats::default().admit("", true), Err(Refusal::Anonymous));
        assert_eq!(Seats::default().admit("a|b", true), Err(Refusal::Anonymous));
    }

    #[test]
    fn a_forged_id_in_the_state_is_dropped() {
        assert_eq!(Seats::parse("x|y", "bb").first(), "");
    }

    #[test]
    fn only_a_player_resets_a_game_in_progress() {
        let s = Seats::parse(A, B);
        assert!(s.may_reset(A, false));
        assert!(!s.may_reset(C, false));
        assert!(s.may_reset(C, true));
        assert!(Seats::parse(A, "").may_reset(C, false));
    }
}
