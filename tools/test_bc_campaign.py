"""BC schedule/source/oracle/accounting tests."""
import json
from pathlib import Path
import tempfile
import unittest
import bc_campaign as bc


class CampaignTests(unittest.TestCase):
    def test_source_variants_change_semantics(self):
        self.assertNotEqual(bc.source('weighted', 0), bc.source('weighted', 7))
        self.assertNotEqual(bc.oracle('weighted', 1000, 0), bc.oracle('weighted', 1000, 7))
        self.assertNotEqual(bc.oracle('scalar', 0, 0), bc.oracle('scalar', 0, 1))
        self.assertIn('await tools.load_quotes(batch_id)', bc.source('weighted', 7))

    def test_schedule(self):
        rows = list(bc.schedule(False))
        self.assertEqual(len(rows), 3 * 8 * 10 * 5)
        self.assertEqual({r[5] for r in rows[:5]}, set(bc.ARMS))
        for case, size in bc.SHAPES:
            requests = [r for r in rows if r[1:3] == (case, size)]
            self.assertEqual({r[4] for r in requests}, set(range(30)))
            self.assertEqual(len({r[6] for r in requests}), 30)
            for variant in range(30):
                matched = [r for r in requests if r[4] == variant]
                self.assertEqual({r[5] for r in matched}, set(bc.ARMS))
                self.assertEqual(len({r[6] for r in matched}), 1)

    def test_missing_and_failed_cannot_summarize(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'raw.jsonl'
            path.write_text(json.dumps({'status': 'failed'}) + '\n')
            with self.assertRaises(AssertionError):
                bc.summarize(path)


if __name__ == '__main__':
    unittest.main()
