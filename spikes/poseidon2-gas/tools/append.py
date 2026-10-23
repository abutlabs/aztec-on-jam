"""#43: appending leaves to an append-only Poseidon2 Merkle tree, two ways.

sequential(): per leaf, rehash its whole path (as a Tornado-style incremental tree does),
updating the frontier. batch(): level by level, as the service's kind 2 does. Both return the
root; check() holds them to a naive full tree at small depths.
"""
import random
import poseidon2 as p2


def zeros(depth):
    z = [0]
    for _ in range(depth - 1):
        z.append(p2.hash([z[-1], z[-1]]))
    return z


def sequential(frontier, zero, index, leaves):
    frontier = list(frontier)
    for k, leaf in enumerate(leaves):
        i, node = index + k, leaf
        for level in range(len(zero)):
            if (i >> level) & 1:
                node = p2.hash([frontier[level], node])
            else:
                frontier[level] = node
                node = p2.hash([node, zero[level]])
    return node


def batch(frontier, zero, start, leaves):
    nodes = list(leaves)
    for level in range(len(zero)):
        if start & 1:
            nodes.insert(0, frontier[level])
            start -= 1
        nodes = [p2.hash([nodes[j], nodes[j + 1] if j + 1 < len(nodes) else zero[level]]) for j in range(0, len(nodes), 2)]
        start >>= 1
    return nodes[0]


def naive(leaves, depth):
    level = list(leaves) + [0] * ((1 << depth) - len(leaves))
    while len(level) > 1:
        level = [p2.hash([level[j], level[j + 1]]) for j in range(0, len(level), 2)]
    return level[0]


def check(depth=5, seed=43):
    """From an empty tree, every split of 2^depth leaves into two appends, against the naive root."""
    rnd = random.Random(seed)
    leaves = [rnd.randrange(p2.P) for _ in range(1 << depth)]
    zero = zeros(depth)
    for a in range(1, (1 << depth)):
        for b in range(1, (1 << depth) - a + 1):
            # the frontier after the first a leaves, by sequential insertion from empty
            frontier = [0] * depth
            for k, leaf in enumerate(leaves[:a]):
                i, node = k, leaf
                for level in range(depth):
                    if (i >> level) & 1:
                        node = p2.hash([frontier[level], node])
                    else:
                        frontier[level] = node
                        node = p2.hash([node, zero[level]])
            want = naive(leaves[:a + b], depth)
            assert sequential(frontier, zero, a, leaves[a:a + b]) == want, (a, b)
            assert batch(frontier, zero, a, leaves[a:a + b]) == want, (a, b)
    return True


if __name__ == "__main__":
    print("depth 4, every split:", check(4))
