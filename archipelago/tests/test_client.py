import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from ootrr.client import NUDGE, PENDING, RETRY, Deliverer, got, recv


class DelivererAsksAgainAfterADroppedGive(unittest.TestCase):
    def test_a_give_dropped_while_paused_is_asked_for_again(self):
        d = Deliverer()
        state = {got("a"): False, got("b"): False, PENDING: False}
        self.assertEqual(d.next(["a", "b"], state, 0.0), "a")
        state[recv("a")] = True
        state[PENDING] = True
        self.assertIsNone(d.next(["a", "b"], state, 0.5))
        state[PENDING] = False
        self.assertIsNone(d.next(["a", "b"], state, 1.0))
        self.assertEqual(d.next(["a", "b"], state, NUDGE), "a")
        self.assertIsNone(d.next(["a", "b"], state, NUDGE + 1.0))
        for t in range(1, 60):
            self.assertIn(d.next(["a", "b"], state, NUDGE * (t + 1)), ("a", None))
        self.assertNotIn("a", d.aside)
        state[recv("a")] = False
        state[got("a")] = True
        self.assertEqual(d.next(["a", "b"], state, 500.0), "b")

    def test_a_give_that_ran_is_not_asked_for_again_before_its_retry(self):
        d = Deliverer()
        state = {got("a"): False, PENDING: False}
        self.assertEqual(d.next(["a"], state, 0.0), "a")
        self.assertIsNone(d.next(["a"], state, RETRY - 1.0))
        self.assertEqual(d.next(["a"], state, RETRY), "a")


if __name__ == "__main__":
    unittest.main()
