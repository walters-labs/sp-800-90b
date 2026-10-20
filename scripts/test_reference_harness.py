import unittest
from reference_harness import allowed_baseline_difference, baseline_values


class BaselinePolicyTests(unittest.TestCase):
    def setUp(self):
        self.name = "ringOsc-nist"
        self.label = "Literal MultiMCW Prediction Estimate: min entropy"
        self.reference = {self.label: 0.29}
        self.actual = {self.label: 0.29 + 3e-10}

    def allowed(self, actual=None, expected=None, mantissa=53, opt_in=True):
        return allowed_baseline_difference(self.name, self.actual if actual is None else actual,
                                           self.reference if expected is None else expected, mantissa, opt_in)

    def test_only_explicit_binary64_allowance(self):
        self.assertTrue(self.allowed())
        self.assertFalse(self.allowed(mantissa=64))
        self.assertFalse(self.allowed(opt_in=False))
        self.assertFalse(self.allowed(actual=self.reference))

    def test_missing_extra_nonfinite_and_excessive_differences_fail(self):
        for actual in [{}, {**self.actual, "extra": 1}, {self.label: float("nan")},
                       {self.label: float("inf")}, {self.label: 0.29 + 2e-9}]:
            self.assertFalse(self.allowed(actual=actual))
        self.assertFalse(self.allowed(actual={}, expected={}))

    def test_unlisted_field_or_dataset_is_not_exempt(self):
        self.assertFalse(self.allowed(actual={"other Estimate:": 0.29 + 3e-10},
                                      expected={"other Estimate:": 0.29}))
        self.assertFalse(allowed_baseline_difference("different", self.actual, self.reference, 53, True))

    def test_parse_only_estimate_fields(self):
        self.assertEqual(baseline_values("warning\nH_original = 1\nLiteral Lag Prediction Estimate: min entropy = 2.5e-1\nAssessed min entropy: = 0.25\n"), {
            "Literal Lag Prediction Estimate: min entropy": 0.25,
            "Assessed min entropy:": 0.25,
        })


if __name__ == "__main__":
    unittest.main()
