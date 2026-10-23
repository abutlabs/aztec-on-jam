#!/usr/bin/env python3
"""Encrypted notes: how a sender tells a receiver about a note through the chain alone.
Stdlib only, and built from primitives WebCrypto also has, so the browser wallet
(demo-wallet `jam/`) seals and opens the same bytes:

  X25519 (RFC 7748)      the sender's ephemeral key against the receiver's encryption key
  HKDF-SHA256 (RFC 5869) a 32-byte encryption key and a 32-byte MAC key from the secret
  SHA-256 counter mode   keystream block i = SHA-256(enc_key || i as u32 BE), XORed in
  HMAC-SHA256            over ephemeral pk || ciphertext, truncated to 16 bytes

A sealed note: ephemeral pk (32) || ciphertext (len(plaintext)) || tag (16). The plaintext
of a note is value (u64 LE) || blinding (32, BE field element) = 40 bytes, so a sealed note
is 88 bytes. A receiver opens every new one with its key; the tag says which are theirs.

This is a demo construction, not a vetted AEAD (decision #20): the way to production is
XChaCha20-Poly1305 or AES-GCM, which the browser and a non-stdlib client both have.
"""
import hashlib, hmac, os

P = 2**255 - 19
A24 = 121665
INFO = b"jam-privacy-note-v1"
TAG_LEN = 16


def _clamp(k: bytes) -> int:
    b = bytearray(k)
    b[0] &= 248; b[31] &= 127; b[31] |= 64
    return int.from_bytes(b, "little")


def x25519(k: bytes, u: bytes) -> bytes:
    """RFC 7748 section 5: the Montgomery ladder on Curve25519."""
    if len(k) != 32 or len(u) != 32:
        raise ValueError("x25519: 32-byte scalar and point")
    k_int = _clamp(k)
    x1 = int.from_bytes(u, "little") & ((1 << 255) - 1)
    x2, z2, x3, z3, swap = 1, 0, x1, 1, 0
    for t in reversed(range(255)):
        kt = (k_int >> t) & 1
        swap ^= kt
        if swap:
            x2, x3, z2, z3 = x3, x2, z3, z2
        swap = kt
        a, b = (x2 + z2) % P, (x2 - z2) % P
        aa, bb = a * a % P, b * b % P
        e = (aa - bb) % P
        c, d = (x3 + z3) % P, (x3 - z3) % P
        da, cb = d * a % P, c * b % P
        x3, z3 = (da + cb) ** 2 % P, x1 * (da - cb) ** 2 % P
        x2, z2 = aa * bb % P, e * (aa + A24 * e) % P
    if swap:
        x2, z2 = x3, z3
    return (x2 * pow(z2, P - 2, P) % P).to_bytes(32, "little")


BASE = (9).to_bytes(32, "little")


def keypair(seed: bytes):
    """(secret, public) from a 32-byte seed."""
    sk = hashlib.sha256(b"jam-privacy-enc-key" + seed).digest()
    return sk, x25519(sk, BASE)


def _hkdf(secret: bytes, salt: bytes, n: int) -> bytes:
    prk = hmac.new(salt, secret, hashlib.sha256).digest()
    out, t, i = b"", b"", 1
    while len(out) < n:
        t = hmac.new(prk, t + INFO + bytes([i]), hashlib.sha256).digest()
        out += t; i += 1
    return out[:n]


def _keystream(key: bytes, n: int) -> bytes:
    return b"".join(hashlib.sha256(key + i.to_bytes(4, "big")).digest() for i in range((n + 31) // 32))[:n]


def seal(receiver_pk: bytes, plaintext: bytes, ephemeral_seed: bytes | None = None) -> bytes:
    esk = hashlib.sha256(b"jam-privacy-ephemeral" + (ephemeral_seed or os.urandom(32))).digest()
    epk = x25519(esk, BASE)
    keys = _hkdf(x25519(esk, receiver_pk), epk, 64)
    ct = bytes(a ^ b for a, b in zip(plaintext, _keystream(keys[:32], len(plaintext))))
    tag = hmac.new(keys[32:], epk + ct, hashlib.sha256).digest()[:TAG_LEN]
    return epk + ct + tag


def open_(receiver_sk: bytes, sealed: bytes) -> bytes | None:
    """The plaintext, or None if this note is not for this key (or was tampered with)."""
    if len(sealed) < 32 + TAG_LEN:
        return None
    epk, ct, tag = sealed[:32], sealed[32:-TAG_LEN], sealed[-TAG_LEN:]
    keys = _hkdf(x25519(receiver_sk, epk), epk, 64)
    if not hmac.compare_digest(hmac.new(keys[32:], epk + ct, hashlib.sha256).digest()[:TAG_LEN], tag):
        return None
    return bytes(a ^ b for a, b in zip(ct, _keystream(keys[:32], len(ct))))


def note_plaintext(value: int, blinding: int) -> bytes:
    return int(value).to_bytes(8, "little") + int(blinding).to_bytes(32, "big")


def note_from_plaintext(pt: bytes):
    return int.from_bytes(pt[:8], "little"), int.from_bytes(pt[8:40], "big")


SEALED_NOTE_LEN = 32 + 40 + TAG_LEN


if __name__ == "__main__":
    # RFC 7748 section 5.2 test vector 1, and the section 6.1 Diffie-Hellman example
    k = bytes.fromhex("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4")
    u = bytes.fromhex("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c")
    assert x25519(k, u).hex() == "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552", "RFC 7748 5.2"
    a_sk = bytes.fromhex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
    b_sk = bytes.fromhex("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb")
    a_pk, b_pk = x25519(a_sk, BASE), x25519(b_sk, BASE)
    assert a_pk.hex() == "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a", "RFC 7748 6.1 Alice"
    assert b_pk.hex() == "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f", "RFC 7748 6.1 Bob"
    assert x25519(a_sk, b_pk) == x25519(b_sk, a_pk) == bytes.fromhex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742")
    # seal and open
    sk, pk = keypair(b"B" * 32)
    other_sk, _ = keypair(b"A" * 32)
    pt = note_plaintext(40, 12345678901234567890)
    s = seal(pk, pt)
    assert len(s) == SEALED_NOTE_LEN and open_(sk, s) == pt and open_(other_sk, s) is None
    bad = bytearray(s); bad[40] ^= 1
    assert open_(sk, bytes(bad)) is None
    assert note_from_plaintext(pt) == (40, 12345678901234567890)
    print("notecrypt: RFC 7748 vectors pass; seal/open, wrong key and tamper checks pass")
