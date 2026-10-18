"""使用合成数据检查发布门禁；不下载或留存真实材料。
Exercise publish gates using synthetic data only.
"""
import base64
import codecs
import hashlib
import os
from pathlib import Path
import tempfile
import unittest

from keybox_sync import MAX_DOWNLOAD, decode_source, prepare, slot_filename

ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = ROOT / "target" / "debug" / ("teeforge.exe" if os.name == "nt" else "teeforge")


def encode_source(xml):
    data = codecs.encode(xml, "rot_13").encode().hex().encode()
    for _ in range(10):
        data = base64.b64encode(data)
    return data


class PublishTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.input = Path(self.temp.name) / "input"
        self.input.mkdir()
        self.output = Path(self.temp.name) / "output"
        self.raw = []
        fixture = (ROOT / "tests/fixtures/keybox-valid.xml").read_text(encoding="utf-8")
        for slot in (1, 2, 3):
            raw = encode_source(fixture.replace("synthetic-test", f"slot-{slot}"))
            self.raw.append(raw)
            (self.input / f"megatron_raw_{slot}").write_bytes(raw)

    def run_prepare(self):
        prepare(self.input, self.output, b"synthetic-key", "2026-10", VALIDATOR)

    def test_all_slots_and_legacy_filename(self):
        self.run_prepare()
        key = hashlib.sha256(b"synthetic-key").digest()
        for slot in (1, 2, 3):
            wrapped = base64.b64decode((self.output / slot_filename("2026-10", slot)).read_bytes(), validate=True)
            raw = bytes(b ^ key[i % len(key)] for i, b in enumerate(wrapped))
            self.assertTrue(raw == self.raw[slot - 1], "slot data differs")
            self.assertTrue(f'slot-{slot}'.encode() in decode_source(raw))
        self.assertEqual(slot_filename("2026-10", 1), hashlib.sha256(b"2026-10").hexdigest()[:12])
        self.assertEqual(len(list(self.output.iterdir())), 17)

    def test_bad_missing_empty_oversized_or_truncated_slot_never_changes_output(self):
        self.output.mkdir()
        existing = self.output / "existing"
        existing.write_bytes(b"old")
        variants = [None, b"", b"x" * (MAX_DOWNLOAD + 1), self.raw[2][:-3], encode_source("<AndroidAttestation>broken"), encode_source("<html>AndroidAttestation</html>")]
        for raw in variants:
            path = self.input / "megatron_raw_3"
            if raw is None:
                path.unlink(missing_ok=True)
            else:
                path.write_bytes(raw)
            with self.assertRaises(Exception):
                self.run_prepare()
            self.assertEqual(list(self.output.iterdir()), [existing])
            self.assertEqual(existing.read_bytes(), b"old")


if __name__ == "__main__":
    unittest.main()
