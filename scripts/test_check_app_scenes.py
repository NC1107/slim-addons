#!/usr/bin/env python3
"""Unit tests for check-app-scenes.py: which frames it asks a module for, and whether it checks them.

Run from the repo root: python3 -m unittest discover -s scripts -p 'test_*.py'
SCRIPTS_DIR points the tests at another copy of the scripts, for mutation checks.
"""
import importlib.util
import json
import os
import pathlib
import unittest

here = pathlib.Path(os.environ.get("SCRIPTS_DIR", pathlib.Path(__file__).resolve().parent))


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, here / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


app_scenes = load("check_app_scenes", "check-app-scenes.py")
scene_ops = load("check_scene_ops", "check-scene-ops.py")


def scene(ops=(), controls=(), state="s1"):
    return json.dumps({"$slim": "scene/1", "ops": list(ops), "controls": list(controls), "state": state})


def actions(raw):
    return [json.loads(p)["action"] for p in app_scenes.follow_ups(raw)]


class FollowUpsTest(unittest.TestCase):
    def test_every_control_tap_and_input_is_offered_with_the_scene_state(self):
        raw = scene(
            ops=[
                {"op": "rect", "tap": "vote:0"},
                {"op": "cells", "tap": "c", "data": "00"},
                {"op": "input", "submit": "guess"},
            ],
            controls=["reset"],
        )
        self.assertEqual(actions(raw), ["reset", "vote:0", "c:0,0", "guess:abcde"])
        self.assertEqual({json.loads(p)["state"] for p in app_scenes.follow_ups(raw)}, {"s1"})

    def test_play_is_sent_as_step_and_repeats_are_dropped(self):
        raw = scene(ops=[{"op": "rect", "tap": "go"}, {"op": "rect", "tap": "go"}], controls=["play", "step"])
        self.assertEqual(actions(raw), ["step", "go"])

    def test_the_number_of_follow_ups_is_bounded(self):
        raw = scene(ops=[{"op": "rect", "tap": f"t{i}"} for i in range(500)])
        self.assertEqual(len(actions(raw)), app_scenes.MAX_FOLLOW_UPS)

    def test_text_that_is_not_a_scene_offers_nothing(self):
        self.assertEqual(actions("plain text"), [])
        self.assertEqual(actions("[1]"), [])


class CheckAppTest(unittest.TestCase):
    def test_a_bad_op_that_only_appears_after_an_action_is_found(self):
        good = scene(ops=[{"op": "rect", "tap": "go"}])
        bad = scene(ops=[{"op": "text", "x": 0, "y": 0, "str": "a", "fill": "txet"}])

        class Fake:
            check_scene = staticmethod(scene_ops.check_scene)
            seen = []

            @classmethod
            def run_module(cls, runner, wasm, command, payload, caller=None):
                cls.seen.append((payload, caller))
                return good if payload == "" else bad

        found, frames = app_scenes.check_app(Fake, "runner", "m.wasm", "play")
        self.assertTrue(found, "the bad frame after the tap was never checked")
        self.assertGreater(frames, 1)
        self.assertEqual({caller for _, caller in Fake.seen}, {"alice", "bob"})


if __name__ == "__main__":
    unittest.main()
