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
        self.assertEqual({r[4] for r in rows if r[1:3] == ('weighted', 100)}, set(range(12)))

    def test_missing_and_failed_cannot_summarize(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'raw.jsonl'
            path.write_text(json.dumps({'status': 'failed'}) + '\n')
            with self.assertRaises(AssertionError):
                bc.summarize(path)


if __name__ == '__main__':
    unittest.main()
