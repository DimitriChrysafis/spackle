import unittest
import tomlite

class TestParser(unittest.TestCase):
    def test_basic(self):
        doc = tomlite.loads('title = "demo"\nport = 8080\n')
        self.assertEqual(doc["title"], "demo")
        self.assertEqual(doc["port"], 8080)

    def test_section(self):
        doc = tomlite.loads('[server]\nhost = "h"\nport = 1\n')
        self.assertEqual(doc["server"]["host"], "h")
        self.assertEqual(doc["server"]["port"], 1)

    def test_nested_section(self):
        doc = tomlite.loads('[a.b]\nx = 1\n')
        self.assertEqual(doc["a"]["b"]["x"], 1)

    def test_comment(self):
        doc = tomlite.loads('# nothing\nport = 1 # trailing\n')
        self.assertEqual(doc["port"], 1)

    def test_hash_inside_string(self):
        doc = tomlite.loads('motd = "welcome #2"\n')
        self.assertEqual(doc["motd"], "welcome #2")

    def test_comment_after_string(self):
        doc = tomlite.loads('motd = "hi #2" # a comment\n')
        self.assertEqual(doc["motd"], "hi #2")

    def test_list(self):
        doc = tomlite.loads('ports = [80, 443]\n')
        self.assertEqual(doc["ports"], [80, 443])

    def test_list_of_strings(self):
        doc = tomlite.loads('tags = ["a,b", "c"]\n')
        self.assertEqual(doc["tags"], ["a,b", "c"])

    def test_bool_float(self):
        doc = tomlite.loads('debug = false\npi = 3.5\n')
        self.assertIs(doc["debug"], False)
        self.assertAlmostEqual(doc["pi"], 3.5)

    def test_escape(self):
        doc = tomlite.loads(r'p = "a\\nb"\n')
        self.assertEqual(doc["p"], "a\nb")

    def test_duplicate_key(self):
        with self.assertRaises(SyntaxError):
            tomlite.loads('a = 1\na = 2\n')

class TestDump(unittest.TestCase):
    def test_round_trip(self):
        src = 'title = "x"\n\n[s]\nn = 2\n'
        doc = tomlite.loads(src)
        self.assertEqual(tomlite.loads(tomlite.dumps(doc)), doc)

