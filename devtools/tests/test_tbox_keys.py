import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))
import _paths  # noqa: E402
import tbox


def fresh():
    return {"buf": "", "early": None, "theld": True}


class PendingKeys(unittest.TestCase):
    def test_first_chars_buffered(self):
        st = fresh()
        st["theld"] = False
        for c, v in (("w", 87), ("h", 72), ("y", 89)):
            tbox.pending_key(st, v, 84, char=c)
        self.assertEqual(st["buf"], "why")

    def test_held_trigger_ignored_then_typed(self):
        st = fresh()
        tbox.pending_key(st, 84, 84, char="t")
        self.assertEqual(st["buf"], "")
        st["theld"] = False
        tbox.pending_key(st, 84, 84, char="t")
        self.assertEqual(st["buf"], "t")

    def test_enter_and_escape_recorded_not_typed(self):
        st = fresh()
        tbox.pending_key(st, 13, 84)
        self.assertEqual((st["early"], st["buf"]), ("send", ""))
        tbox.pending_key(st, 27, 84)
        self.assertEqual(st["early"], "cancel")

    def test_backspace_and_modifiers(self):
        st = fresh()
        tbox.pending_key(st, 65, 84, char="a")
        tbox.pending_key(st, 0x10, 84, char="X")
        tbox.pending_key(st, 8, 84)
        tbox.pending_key(st, 8, 84)
        self.assertEqual(st["buf"], "")


class Real(unittest.TestCase):
    def test_key_char(self):
        self.assertEqual(tbox.key_char(ord("A"), 0x1E).lower(), "a")


if __name__ == "__main__":
    unittest.main()
