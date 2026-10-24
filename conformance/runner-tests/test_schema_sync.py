#!/usr/bin/env python3
"""The JSON Schemas under schema/ must agree with the executable validator."""
import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SUITE = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(SUITE, "runner"))
from ocs import model  # noqa: E402
from ocs.judge import ADAPTER_OUTCOMES  # noqa: E402


def load(name):
    with open(os.path.join(SUITE, "schema", name + ".schema.json")) as fh:
        return json.load(fh)


class SchemaSync(unittest.TestCase):
    def test_case_enums(self):
        s = load("case")["properties"]
        self.assertEqual(s["mode"]["enum"], list(model.MODES))
        self.assertEqual(s["status"]["enum"], list(model.CASE_STATUSES))
        self.assertEqual(s["expect"]["properties"]["outcome"]["enum"], list(model.OUTCOMES))
        self.assertEqual(s["requires"]["properties"]["environment"]["items"]["enum"], list(model.ENVIRONMENT_TAGS))
        self.assertEqual(s["contracts"]["items"]["properties"]["aspects"]["items"]["enum"], list(model.ASPECTS))
        self.assertEqual(set(load("case")["required"]), set(model.CASE_KEYS_REQUIRED))

    def test_response_outcomes(self):
        self.assertEqual(load("adapter-response")["properties"]["outcome"]["enum"], list(ADAPTER_OUTCOMES))

    def test_every_case_matches_required_keys(self):
        required = set(load("case")["required"])
        allowed = set(load("case")["properties"])
        for dirpath, _, files in os.walk(os.path.join(SUITE, "cases")):
            if "case.json" in files:
                with open(os.path.join(dirpath, "case.json")) as fh:
                    data = json.load(fh)
                self.assertTrue(required <= set(data), dirpath)
                self.assertTrue(set(data) <= allowed, dirpath)


if __name__ == "__main__":
    unittest.main(verbosity=1)
