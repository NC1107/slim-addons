#!/usr/bin/env python3
"""Unit tests for check-scene-ops.py: the contract table it keeps for the renderer.

Run from the repo root: python3 -m unittest discover -s scripts -p 'test_*.py'
SCRIPTS_DIR points the tests at another copy of the scripts, for mutation checks.
"""
import importlib.util
import json
import os
import pathlib
import unittest

here = pathlib.Path(os.environ.get("SCRIPTS_DIR", pathlib.Path(__file__).resolve().parent))
spec = importlib.util.spec_from_file_location("check_scene_ops", here / "check-scene-ops.py")
scene_ops = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scene_ops)


def scene(*ops, **extra):
    return json.dumps({"$slim": "scene/1", "ops": list(ops), **extra})


class CheckSceneTest(unittest.TestCase):
    def test_a_good_scene_has_no_problems(self):
        good = scene({"op": "rect", "x": 0, "y": 0, "w": 5, "h": 5, "fill": "#abc"}, background="surface")
        self.assertEqual(scene_ops.check_scene(good), [])

    def test_sweep_is_read_by_the_four_ops_that_move(self):
        sweep = {"secs": 1, "dx": 5}
        ops = [
            {"op": "rect", "x": 0, "y": 0, "w": 1, "h": 1, "sweep": sweep},
            {"op": "circle", "cx": 0, "cy": 0, "r": 1, "sweep": sweep},
            {"op": "line", "x1": 0, "y1": 0, "x2": 1, "y2": 1, "sweep": sweep},
            {"op": "text", "x": 0, "y": 0, "s": "a", "sweep": sweep},
        ]
        self.assertEqual(scene_ops.check_scene(scene(*ops)), [])

    def test_a_ninth_sweep_is_reported(self):
        ops = [{"op": "rect", "x": i, "y": 0, "w": 1, "h": 1, "sweep": {"secs": 1}} for i in range(9)]
        problems = scene_ops.check_scene(scene(*ops))
        self.assertEqual(len(problems), 1)
        self.assertIn("more than 8 sweeps", problems[0])

    def test_a_malformed_hex_colour_is_reported(self):
        for bad in ("#12", "#zzzzzz", "#12345", "#"):
            op = {"op": "rect", "x": 0, "y": 0, "w": 1, "h": 1, "fill": bad}
            self.assertEqual(len(scene_ops.check_scene(scene(op))), 1, bad)
        for good in ("#123", "#112233", "#11223344"):
            op = {"op": "rect", "x": 0, "y": 0, "w": 1, "h": 1, "fill": good}
            self.assertEqual(scene_ops.check_scene(scene(op)), [], good)

    def test_a_bad_background_is_a_message_not_a_crash(self):
        self.assertEqual(len(scene_ops.check_scene(scene(background=7))), 1)
        self.assertEqual(len(scene_ops.check_scene(scene(background="#12"))), 1)

    def test_a_non_object_op_or_scene_is_a_message_not_a_crash(self):
        self.assertEqual(len(scene_ops.check_scene(scene(5))), 1)
        self.assertEqual(len(scene_ops.check_scene("[1, 2]")), 1)

    def test_a_path_over_the_step_cap_is_reported(self):
        long = "M0 0 " + "L1 1 " * 600
        problems = scene_ops.check_scene(scene({"op": "path", "d": long}))
        self.assertEqual(len(problems), 1)
        self.assertIn("512 path steps", problems[0])

    def test_a_path_at_the_cap_passes(self):
        exact = "M0 0 " + "L1 1 " * 510 + "Z"
        self.assertEqual(scene_ops.check_scene(scene({"op": "path", "d": exact})), [])

    def test_implicit_repeats_count_as_steps(self):
        self.assertEqual(scene_ops.path_steps("M0 0 1 1 2 2 3 3"), 4)
        self.assertEqual(scene_ops.path_steps("M0 0 H5 V5 Z"), 4)


if __name__ == "__main__":
    unittest.main()
