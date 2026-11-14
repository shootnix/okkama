#!/usr/bin/env python3
"""The Rust adapter must not reuse another engine's declared capabilities."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


path = Path(__file__).resolve().parents[1] / "adapters/rust/okkama_rust_adapter.py"
spec = importlib.util.spec_from_file_location("rust_adapter", path)
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)


class ProbeCacheTests(unittest.TestCase):
    def test_default_engine_is_sparrow(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(adapter.os.environ, {
                "OCS_RUST_IMPLEMENTATION_ROOT": directory,
                "OCS_RUST_TARGET_DIR": directory,
            }), patch.object(adapter, "source_fingerprint", return_value="same-source"), \
                    patch.object(adapter, "probe_uncached", return_value={"build_exit": 0, "modes": ["run"]}) as build:
                adapter.os.environ.pop("OCS_RUST_ENGINE", None)
                self.assertEqual(adapter.probe()["engine"], "sparrow")
                self.assertTrue(adapter.probe()["cached"])
                self.assertEqual(build.call_count, 1)

    def test_removed_engine_is_rejected(self):
        with patch.dict(adapter.os.environ, {"OCS_RUST_ENGINE": "ir"}):
            with self.assertRaisesRegex(SystemExit, "Sparrow is the only"):
                adapter.probe()


if __name__ == "__main__":
    unittest.main()
