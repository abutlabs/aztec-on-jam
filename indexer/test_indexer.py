"""The indexer's placement of private transfers in the slots they landed in, on a fake chain.

    python3 -m unittest indexer/test_indexer.py
"""
import importlib.util, os, struct, unittest

os.environ.setdefault("PRIVACY_ID", "2"); os.environ.setdefault("PUBLIC_ID", "1")
# by its path: the folder shares the module's name
_spec = importlib.util.spec_from_file_location("indexer_main", os.path.join(os.path.dirname(__file__), "indexer.py"))
indexer = importlib.util.module_from_spec(_spec); _spec.loader.exec_module(indexer)


class Chain:
    """Blocks at slots 1.., each with the privacy service's transfer count after it."""
    def __init__(self, counts, fail_reads=0):
        self.counts = counts                     # counts[i]: transfers after the block at slot i+1
        self.fail_reads = fail_reads             # how many reads fail before they work

    def head(self):
        n = len(self.counts)
        return n, self.hash(n)

    def hash(self, slot):
        return struct.pack("<I", slot)

    def parent(self, at):
        if self.fail_reads:
            self.fail_reads -= 1
            raise ConnectionError("the reader went away")
        s = struct.unpack("<I", at)[0]
        return None if s <= 1 else (s - 1, self.hash(s - 1))

    def read(self, service, key, at=None):
        return struct.pack("<Q", self.counts[struct.unpack("<I", at)[0] - 1])


class PlaceSlots(unittest.TestCase):
    def use(self, chain):
        indexer.B = chain
        slot, at = chain.head()
        return slot, at

    def test_each_transfer_gets_the_slot_its_count_rose_in(self):
        # slots 1..5: transfers 0, 1, 1, 3, 4 -> transfer 0 at slot 2, 1 and 2 at slot 4, 3 at slot 5
        slot, at = self.use(Chain([0, 1, 1, 3, 4]))
        self.assertEqual(indexer.place_slots([], 4, slot, at), [2, 4, 4, 5])

    def test_a_failed_read_leaves_them_unplaced_and_a_later_refresh_places_them(self):
        chain = Chain([0, 1, 1, 3, 4], fail_reads=1)
        slot, at = self.use(chain)
        first = indexer.place_slots([], 4, slot, at)
        self.assertEqual(first, [], "a failed read must not mark transfers unknown for good")
        self.assertEqual(indexer.place_slots(first, 4, slot, at), [2, 4, 4, 5])

    def test_transfers_beyond_the_walk_are_unknown(self):
        old = indexer.WALK
        indexer.WALK = 2
        try:
            # transfer 0 landed at slot 2, more than two blocks back from the head at slot 5
            slot, at = self.use(Chain([0, 1, 1, 3, 4]))
            self.assertEqual(indexer.place_slots([], 4, slot, at), [None, 4, 4, 5])
        finally:
            indexer.WALK = old

    def test_only_new_transfers_are_placed(self):
        slot, at = self.use(Chain([0, 1, 1, 3, 4]))
        self.assertEqual(indexer.place_slots([2, 4], 4, slot, at), [2, 4, 4, 5])
        self.assertEqual(indexer.place_slots([2, 4, 4, 5], 4, slot, at), [2, 4, 4, 5])


if __name__ == "__main__":
    unittest.main()
