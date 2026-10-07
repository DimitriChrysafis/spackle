import unittest
from tinytmpl import render


class T(unittest.TestCase):
    def test_var(self):
        self.assertEqual(render("hi {{name}}", {"name": "bo"}), "hi bo")

    def test_missing_var(self):
        self.assertEqual(render("[{{x}}]", {}), "[]")

    def test_dot_lookup(self):
        self.assertEqual(render("{{u.name}}", {"u": {"name": "al"}}), "al")

    def test_each_dot(self):
        out = render("{{#each items}}{{.}},{{/each}}", {"items": [1, 2, 3]})
        self.assertEqual(out, "1,2,3,")

    def test_each_dict_item(self):
        out = render(
            "{{#each users}}{{name}}={{age}};{{/each}}",
            {"users": [{"name": "a", "age": 1}, {"name": "b", "age": 2}]},
        )
        self.assertEqual(out, "a=1;b=2;")

    def test_index(self):
        out = render("{{#each xs}}{{@index}}:{{.}} {{/each}}", {"xs": ["a", "b"]})
        self.assertEqual(out, "0:a 1:b ")

    def test_if(self):
        self.assertEqual(render("{{#if ok}}yes{{/if}}", {"ok": 1}), "yes")
        self.assertEqual(render("{{#if ok}}yes{{/if}}", {}), "")

    def test_nested_each(self):
        src = "{{#each groups}}{{name}}:{{#each items}}{{.}}{{/each}};{{/each}}"
        ctx = {
            "groups": [
                {"name": "g1", "items": ["a", "b"]},
                {"name": "g2", "items": ["c"]},
            ]
        }
        self.assertEqual(render(src, ctx), "g1:ab;g2:c;")

    def test_inner_loop_reads_outer_scope(self):
        # inside the inner each, `name` still refers to the outer group
        src = "{{#each groups}}{{#each items}}{{name}}={{.}} {{/each}}{{/each}}"
        ctx = {"groups": [{"name": "g", "items": ["x", "y"]}]}
        self.assertEqual(render(src, ctx), "g=x g=y ")

    def test_global_inside_each(self):
        out = render(
            "{{#each xs}}{{title}}:{{.}} {{/each}}",
            {"title": "T", "xs": ["a"]},
        )
        self.assertEqual(out, "T:a ")

    def test_unclosed(self):
        with self.assertRaises(SyntaxError):
            render("{{#each xs}}", {"xs": []})
