#!/usr/bin/env python3
"""Encrypted notes: how a sender tells a receiver about a note through the chain alone.
Built only from primitives WebCrypto also has, so a browser wallet seals and opens the same
bytes (the first demo's did; branch archive/first-demo):

  X25519 (RFC 7748)          the sender's fresh ephemeral key against the receiver's key
  HKDF-SHA256 (RFC 5869)     a 32-byte AES key, salted with the ephemeral public key
  AES-256-GCM                the note's opening, with the ephemeral key as associated data

Every note gets a fresh ephemeral key, so every AES key is used once and the nonce can be
fixed (12 zero bytes). A sealed note: ephemeral pk (32) || ciphertext (40) || tag (16) = 88
bytes. A receiver tries every new one with its key; the tag says which are theirs. Aztec
seals notes the same way in outline (ECDH, then AES), on the Grumpkin curve.

Needs the `cryptography` package (scripts/bootstrap.sh installs it).
"""
import hashlib, os, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "out" / "pydeps"))   # bootstrap's --target
from cryptography.exceptions import InvalidTag  # noqa: E402
from cryptography.hazmat.primitives import hashes  # noqa: E402
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey  # noqa: E402
from cryptography.hazmat.primitives.ciphers.aead import AESGCM  # noqa: E402
from cryptography.hazmat.primitives.kdf.hkdf import HKDF  # noqa: E402
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat  # noqa: E402

INFO = b"jam-privacy-note-v1"
NONCE = bytes(12)
SEALED_NOTE_LEN = 32 + 40 + 16


def _pub(sk: X25519PrivateKey) -> bytes:
    return sk.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)


def x25519(k: bytes, u: bytes) -> bytes:
    return X25519PrivateKey.from_private_bytes(k).exchange(X25519PublicKey.from_public_bytes(u))


def keypair(seed: bytes):
    """(secret, public), 32 bytes each, from a seed: a user's note-encryption key."""
    sk = hashlib.sha256(b"jam-privacy-enc-key" + seed).digest()
    return sk, _pub(X25519PrivateKey.from_private_bytes(sk))


def _key(shared: bytes, epk: bytes) -> bytes:
    return HKDF(algorithm=hashes.SHA256(), length=32, salt=epk, info=INFO).derive(shared)


def seal(receiver_pk: bytes, plaintext: bytes) -> bytes:
    esk = X25519PrivateKey.generate()
    epk = _pub(esk)
    key = _key(esk.exchange(X25519PublicKey.from_public_bytes(receiver_pk)), epk)
    return epk + AESGCM(key).encrypt(NONCE, plaintext, epk)


def open_(receiver_sk: bytes, sealed: bytes) -> bytes | None:
    """The plaintext, or None if this note is not for this key (or was tampered with)."""
    if len(sealed) < 32 + 16:
        return None
    epk = sealed[:32]
    try:
        key = _key(x25519(receiver_sk, epk), epk)
        return AESGCM(key).decrypt(NONCE, sealed[32:], epk)
    except (InvalidTag, ValueError):
        return None


def note_plaintext(value: int, blinding: int) -> bytes:
    return int(value).to_bytes(8, "little") + int(blinding).to_bytes(32, "big")


def note_from_plaintext(pt: bytes):
    return int.from_bytes(pt[:8], "little"), int.from_bytes(pt[8:40], "big")


if __name__ == "__main__":
    # RFC 7748 section 5.2 test vector 1 and the section 6.1 Diffie-Hellman example
    k = bytes.fromhex("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4")
    u = bytes.fromhex("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c")
    assert x25519(k, u).hex() == "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552", "RFC 7748 5.2"
    a_sk = bytes.fromhex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
    b_sk = bytes.fromhex("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb")
    a_pk, b_pk = _pub(X25519PrivateKey.from_private_bytes(a_sk)), _pub(X25519PrivateKey.from_private_bytes(b_sk))
    assert a_pk.hex() == "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a", "RFC 7748 6.1 Alice"
    assert x25519(a_sk, b_pk).hex() == "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742", "RFC 7748 6.1"
    sk, pk = keypair(b"B" * 32)
    other_sk, _ = keypair(b"A" * 32)
    pt = note_plaintext(40, 12345678901234567890)
    s = seal(pk, pt)
    assert len(s) == SEALED_NOTE_LEN and open_(sk, s) == pt and open_(other_sk, s) is None
    bad = bytearray(s); bad[40] ^= 1
    assert open_(sk, bytes(bad)) is None
    assert note_from_plaintext(pt) == (40, 12345678901234567890)
    print("notecrypt: RFC 7748 vectors pass; AES-256-GCM seal/open, wrong key and tamper checks pass")
