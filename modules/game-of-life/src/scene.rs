//! Turns a [`Board`] into a slim scene: the JSON envelope the client sniffs
//! for and paints instead of showing as text (see the client's
//! `module_scene.dart`). This module owns none of that contract's meaning; it
//! only fills in the one `cells` grid, the control set and the caption that a
//! Game of Life needs, in colours named for the host's own theme tokens so the
//! board looks native in light and dark alike.

use serde::Serialize;

use crate::life::{Board, COLS, ROWS};

#[derive(Serialize)]
pub struct Scene {
    #[serde(rename = "$slim")]
    schema: &'static str,
    width: usize,
    height: usize,
    background: &'static str,
    ops: Vec<CellsOp>,
    state: String,
    controls: Vec<&'static str>,
    status: String,
    live: bool,
}

#[derive(Serialize)]
struct CellsOp {
    op: &'static str,
    cols: usize,
    rows: usize,
    data: String,
    palette: Vec<&'static str>,
    gap: f32,
    tap: &'static str,
}

impl Scene {
    /// Builds the scene for `board`. `live` says whether the world can still
    /// change; the client stops its play timer when it cannot, so a settled or
    /// empty board comes to rest instead of spinning forever.
    pub fn of(board: &Board, live: bool) -> Self {
        let population = board.population();
        Scene {
            schema: "scene/1",
            width: COLS,
            height: ROWS,
            background: "surface",
            ops: vec![CellsOp {
                op: "cells",
                cols: COLS,
                rows: ROWS,
                data: board.cell_string(),
                palette: vec!["sunken", "accent"],
                gap: 0.08,
                tap: "toggle",
            }],
            state: board.to_state(),
            controls: vec!["play", "step", "random", "clear"],
            status: format!("Generation {} · {} alive", board.gen, population),
            live: live && population > 0,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|_| String::from(r#"{"ok":false}"#))
    }
}
