//! Poll: create a poll, tap a bar to vote, tap "close poll" to lock it.
//!
//! Every frame is a fresh sandboxed call. The whole poll rides in the scene's
//! opaque `state` as `<open>\x1e<question>\x1e<label>\x1fvotes\x1e<label>\x1fvotes...`,
//! and nothing is remembered on the host between calls - the same shape every
//! other scene module here uses.
//!
//! One thing this module cannot do, and no scene module can: know who tapped.
//! The request a module receives is `{command, input}` and nothing else - no
//! caller id, no channel id - so "one vote per person" is not enforceable
//! here. Closing the poll bounds the damage (nobody can pad the count once a
//! result is locked in), but while it is open, the same finger can tap a bar
//! as many times as it likes. A capability that only *told* a module who was
//! calling, short of letting it act on their behalf, would fix this; nothing
//! that narrow exists yet (see slim-m's docs/decisions/0023).

use serde::Deserialize;
use serde_json::{json, Value};

const FIELD: char = '\u{1e}'; // separates open-flag / question / options-blob
const ITEM: char = '\u{1f}'; // separates one option's label from its count
const ENTRY: char = '\u{1d}'; // separates option entries within the blob

const MIN_OPTIONS: usize = 2;
const MAX_OPTIONS: usize = 6;
const MAX_QUESTION: usize = 60;
const MAX_OPTION: usize = 20;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "poll" => Ok(handle(input)),
        other => Err(format!("unknown command: {other}")),
    }
}

fn handle(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return render_setup(None);
    }
    match serde_json::from_str::<Action>(trimmed) {
        Ok(action) => dispatch(action),
        // Not an interactive frame: a slash-command launch hands the typed
        // text straight through, so treat it as a poll definition.
        Err(_) => match parse_setup(trimmed) {
            Ok(poll) => render_poll(&poll, None),
            Err(msg) => render_setup(Some(&msg)),
        },
    }
}

fn dispatch(action: Action) -> String {
    let existing = Poll::parse(&action.state);

    if let Some(rest) = action.action.strip_prefix("setup:") {
        return match parse_setup(rest) {
            Ok(poll) => render_poll(&poll, None),
            Err(msg) => render_setup(Some(&msg)),
        };
    }
    if action.action == "reset" {
        return render_setup(None);
    }
    if let Some(rest) = action.action.strip_prefix("vote:") {
        return match existing {
            Some(mut poll) if poll.open => {
                if let Some(option) = rest
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| poll.options.get_mut(i))
                {
                    option.votes = option.votes.saturating_add(1);
                }
                render_poll(&poll, None)
            }
            Some(poll) => render_poll(&poll, Some("poll is closed")),
            None => render_setup(Some("start a poll first")),
        };
    }
    if action.action == "close" {
        return match existing {
            Some(mut poll) => {
                poll.open = false;
                render_poll(&poll, None)
            }
            None => render_setup(None),
        };
    }
    // Unknown action: redraw whatever is currently there rather than guessing.
    match existing {
        Some(poll) => render_poll(&poll, None),
        None => render_setup(None),
    }
}

#[derive(Deserialize)]
struct Action {
    action: String,
    #[serde(default)]
    state: String,
}

struct OptionTally {
    label: String,
    votes: u32,
}

struct Poll {
    open: bool,
    question: String,
    options: Vec<OptionTally>,
}

impl Poll {
    fn parse(state: &str) -> Option<Self> {
        let mut fields = state.split(FIELD);
        let open = fields.next()? == "1";
        let question = fields.next()?.to_string();
        let blob = fields.next().unwrap_or("");
        if question.is_empty() {
            return None;
        }
        let options: Vec<OptionTally> = blob
            .split(ENTRY)
            .filter(|entry| !entry.is_empty())
            .filter_map(|entry| {
                let (label, votes) = entry.split_once(ITEM)?;
                Some(OptionTally {
                    label: label.to_string(),
                    votes: votes.parse().unwrap_or(0),
                })
            })
            .collect();
        if options.len() < MIN_OPTIONS {
            return None;
        }
        Some(Poll {
            open,
            question,
            options,
        })
    }

    fn state(&self) -> String {
        let blob = self
            .options
            .iter()
            .map(|o| format!("{}{}{}", o.label, ITEM, o.votes))
            .collect::<Vec<_>>()
            .join(&ENTRY.to_string());
        format!(
            "{}{FIELD}{}{FIELD}{}",
            if self.open { "1" } else { "0" },
            self.question,
            blob
        )
    }

    fn total_votes(&self) -> u32 {
        self.options.iter().map(|o| o.votes).sum()
    }
}

/// Sanitizes free text so it can never collide with the state's own
/// delimiters, and bounds it so a poll stays a small drawing rather than an
/// ever-growing one.
fn sanitize(text: &str, max_chars: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect()
}

/// Parses `"<question> | <opt a>, <opt b>, ..."` into a fresh, open poll.
fn parse_setup(text: &str) -> Result<Poll, String> {
    let (question_part, options_part) = text
        .split_once('|')
        .ok_or_else(|| "use: question? | option a, option b".to_string())?;

    let question = sanitize(question_part.trim(), MAX_QUESTION);
    if question.is_empty() {
        return Err("need a question before the |".to_string());
    }

    let options: Vec<OptionTally> = options_part
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| OptionTally {
            label: sanitize(s, MAX_OPTION),
            votes: 0,
        })
        .filter(|o| !o.label.is_empty())
        .collect();

    if options.len() < MIN_OPTIONS {
        return Err(format!("need at least {MIN_OPTIONS} options after the |"));
    }
    if options.len() > MAX_OPTIONS {
        return Err(format!("use at most {MAX_OPTIONS} options"));
    }

    Ok(Poll {
        open: true,
        question,
        options,
    })
}

fn render_setup(refused: Option<&str>) -> String {
    let status = refused
        .map(str::to_string)
        .unwrap_or_else(|| "type a question and 2 to 6 options, separated by |".to_string());
    json!({
        "$slim": "scene/1",
        "width": 100,
        "height": 60,
        "background": "surface",
        "ops": [
            { "op": "text", "x": 50.0, "y": 14.0, "s": "new poll", "fill": "text", "align": "center", "size": 10.0 },
            { "op": "input", "x": 10.0, "y": 26.0, "w": 80.0, "submit": "setup", "placeholder": "question? | opt a, opt b", "max": 200 }
        ],
        "status": status,
        "controls": [],
        "state": "",
    })
    .to_string()
}

fn render_poll(poll: &Poll, note: Option<&str>) -> String {
    let total = poll.total_votes();
    let mut ops: Vec<Value> = vec![json!({
        "op": "text", "x": 50.0, "y": 8.0, "s": poll.question, "fill": "text", "align": "center", "size": 7.0
    })];

    for (i, option) in poll.options.iter().enumerate() {
        let y = 16.0 + (i as f64) * 13.0;
        let pct = if total > 0 {
            option.votes as f64 / total as f64
        } else {
            0.0
        };
        let bar_w = 80.0 * pct;

        let mut track = json!({
            "op": "rect", "x": 10.0, "y": y, "w": 80.0, "h": 10.0, "r": 2.0, "fill": "sunken"
        });
        if poll.open {
            track["tap"] = json!(format!("vote:{i}"));
        }
        ops.push(track);

        if bar_w > 0.5 {
            ops.push(json!({
                "op": "rect", "x": 10.0, "y": y, "w": bar_w, "h": 10.0, "r": 2.0,
                "grad": { "from": "accent", "to": "accent-soft", "dir": "h" }
            }));
        }

        ops.push(json!({
            "op": "text", "x": 13.0, "y": y + 7.0, "s": option.label, "fill": "text", "align": "left", "size": 5.5
        }));
        let pct_label = format!("{} ({}%)", option.votes, (pct * 100.0).round() as i64);
        ops.push(json!({
            "op": "text", "x": 87.0, "y": y + 7.0, "s": pct_label, "fill": "text", "align": "right", "size": 5.5
        }));
    }

    let status = match note {
        Some(msg) => msg.to_string(),
        None if poll.open => format!("{total} vote(s) - tap a bar to vote"),
        None => format!("final: {total} vote(s)"),
    };

    let controls: Vec<&str> = if poll.open {
        vec!["close poll", "reset"]
    } else {
        vec!["reset"]
    };
    let height = 16.0 + (poll.options.len() as f64) * 13.0 + 6.0;

    json!({
        "$slim": "scene/1",
        "width": 100,
        "height": height,
        "background": "surface",
        "ops": ops,
        "status": status,
        "controls": controls,
        "state": poll.state(),
        "live": false,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_the_setup_screen() {
        let out = apply("poll", "").unwrap();
        assert!(out.contains(r#""$slim":"scene/1""#));
        assert!(out.contains("new poll"));
    }

    #[test]
    fn a_slash_command_style_string_creates_a_poll_directly() {
        let out = apply("poll", "tabs or spaces? | tabs, spaces").unwrap();
        assert!(out.contains("tabs or spaces?"));
        assert!(out.contains(r#""controls":["close poll","reset"]"#));
    }

    #[test]
    fn setup_needs_a_pipe_and_at_least_two_options() {
        assert!(parse_setup("no pipe here").is_err());
        assert!(parse_setup("q? | onlyone").is_err());
        assert!(parse_setup("q? | a, b").is_ok());
    }

    #[test]
    fn setup_rejects_too_many_options() {
        let text = "q? | a, b, c, d, e, f, g";
        assert!(parse_setup(text).is_err());
    }

    #[test]
    fn voting_increments_the_right_option_and_state_round_trips() {
        let poll = parse_setup("pizza or tacos? | pizza, tacos").unwrap();
        let state = poll.state();
        let action = Action {
            action: "vote:1".to_string(),
            state,
        };
        let out = dispatch(action);
        let reparsed = Poll::parse(&extract_state(&out)).unwrap();
        assert_eq!(reparsed.options[0].votes, 0);
        assert_eq!(reparsed.options[1].votes, 1);
    }

    #[test]
    fn closing_stops_further_votes_from_landing() {
        let mut poll = parse_setup("q? | a, b").unwrap();
        poll.options[0].votes = 3;
        let closed_state = {
            poll.open = false;
            poll.state()
        };
        let action = Action {
            action: "vote:0".to_string(),
            state: closed_state,
        };
        let out = dispatch(action);
        let reparsed = Poll::parse(&extract_state(&out)).unwrap();
        assert_eq!(
            reparsed.options[0].votes, 3,
            "a closed poll must not accept a vote"
        );
        assert!(out.contains("poll is closed"));
    }

    #[test]
    fn reset_discards_the_poll_and_returns_to_setup() {
        let action = Action {
            action: "reset".to_string(),
            state: String::new(),
        };
        let out = dispatch(action);
        assert!(out.contains("new poll"));
    }

    #[test]
    fn a_question_or_option_cannot_smuggle_the_state_delimiters() {
        let evil_question = format!("evil{FIELD}question{ENTRY}{ITEM}?");
        let poll = parse_setup(&format!("{evil_question} | a, b")).unwrap();
        let state = poll.state();
        let reparsed =
            Poll::parse(&state).expect("state must still parse after a hostile question");
        assert_eq!(reparsed.options.len(), 2);
    }

    fn extract_state(scene_json: &str) -> String {
        let value: Value = serde_json::from_str(scene_json).unwrap();
        value["state"].as_str().unwrap().to_string()
    }
}
