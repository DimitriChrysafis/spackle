import json
import os
import tempfile
import unittest
from envcfg import load

class T(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        for var in ("APP_PORT", "APP_HOST", "APP_DEBUG"):
            os.environ.pop(var, None)

    def _cfg(self, obj):
        p = os.path.join(self.tmp, "cfg.json")
        with open(p, "w") as fh:
            json.dump(obj, fh)
        return p

    def _env(self, text):
        p = os.path.join(self.tmp, ".env")
        with open(p, "w") as fh:
            fh.write(text)
        return p

    def test_defaults(self):
        c = load(defaults={"port": "8080"})
        self.assertEqual(c.get_int("port"), 8080)

    def test_file_beats_defaults(self):
        p = self._cfg({"port": "9090"})
        c = load(defaults={"port": "8080"}, config_file=p)
        self.assertEqual(c.get_int("port"), 9090)

    def test_dotenv_beats_file(self):
        cfg = self._cfg({"port": "9090"})
        env = self._env("PORT=7777\n")
        c = load(config_file=cfg, dotenv_file=env)
        self.assertEqual(c.get_int("port"), 7777)

    def test_env_beats_dotenv_and_file(self):
        os.environ["APP_PORT"] = "1234"
        cfg = self._cfg({"port": "9090"})
        env = self._env("PORT=7777\n")
        c = load(config_file=cfg, dotenv_file=env)
        self.assertEqual(c.get_int("port"), 1234)

    def test_typed(self):
        c = load(defaults={"debug": "on", "n": "42"})
        self.assertTrue(c.get_bool("debug"))
        self.assertEqual(c.get_int("n"), 42)

    def test_require(self):
        c = load(defaults={})
        with self.assertRaises(KeyError):
            c.require("missing")
