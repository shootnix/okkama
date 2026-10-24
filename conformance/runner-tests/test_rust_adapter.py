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
    def test_engine_change_invalidates_cached_capabilities(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(adapter.os.environ, {
                "OCS_RUST_IMPLEMENTATION_ROOT": directory,
                "OCS_RUST_TARGET_DIR": directory,
                "OCS_RUST_ENGINE": "ir",
            }), patch.object(adapter, "source_fingerprint", return_value="same-source"), \
                    patch.object(adapter, "probe_uncached", return_value={"modes": ["run"]}) as build:
                self.assertEqual(adapter.probe()["engine"], "ir")
                self.assertTrue(adapter.probe()["cached"])
                self.assertEqual(build.call_count, 1)
                adapter.os.environ["OCS_RUST_ENGINE"] = "jvm"
                build.return_value = {"modes": ["run", "test"]}
                self.assertIn("test", adapter.probe()["modes"])
                self.assertEqual(build.call_count, 2)
                adapter.os.environ["OCS_RUST_ENGINE"] = "sparrow"
                build.return_value = {"modes": ["run"]}
                self.assertNotIn("test", adapter.probe()["modes"])
                self.assertEqual(build.call_count, 3)
                adapter.os.environ["OCS_RUST_ENGINE"] = "jvm"
                self.assertIn("test", adapter.probe()["modes"])
                self.assertTrue(adapter.probe()["cached"])
                self.assertEqual(build.call_count, 3)


if __name__ == "__main__":
    unittest.main()
