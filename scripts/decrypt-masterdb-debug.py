"""Decrypts a Rekordbox `master.db` copy into a plain SQLite file for ad-hoc
inspection (grep/sqlite3/Python queries) — a debug aid, not part of the app.

Pure-Python port of `decrypt_masterdb()`/`deobfuscate_key()` in
`src-tauri/src/rekordbox_masterdb.rs` (SQLCipher v4: PBKDF2-HMAC-SHA512 key
derivation, AES-256-CBC page decryption, HMAC-SHA512 page verification). Built
2026-07-10 to cross-reference which tracks a real `master.db` copy actually
contains against Sift's own `tracks.path` — while debugging why M8 Tier 1/3
candidate detection produced nothing on tracks that turned out to live at a
different physical path in Rekordbox (see memory
`sift-rekordbox-path-separator-mismatch`, and the "Marmite On The Keys"/"Mona
Bone" false alarms in that session's transcript). The Rust reader is the
source of truth and stays the only thing the app itself uses — this script
exists purely so a human/agent can eyeball the decrypted content without
writing a throwaway Rust test each time.

**Never point this at a live Pioneer folder.** Always a disposable copy —
same convention as every other M8 real-copy test (`SIFT_M8_REAL_COPY_DIR`).

Usage:
    python scripts/decrypt-masterdb-debug.py <path/to/master.db> <output.sqlite>
    sqlite3 output.sqlite "SELECT ID, FolderPath FROM djmdContent LIMIT 20"

Requires: pip install pycryptodome
"""
import base64
import sys
import zlib
from Crypto.Cipher import AES
from Crypto.Hash import HMAC, SHA512
from Crypto.Protocol.KDF import PBKDF2

# Same static, publicly documented blob as rekordbox_masterdb.rs's BLOB/BLOB_KEY —
# obfuscated passphrase, not a per-installation secret. Reversed via base85
# (RFC1924 alphabet, matches CPython's base64.b85decode) + XOR + zlib.
BLOB = "PN_Pq^*N>(JYe*u^8;Yg76HuZ<mR13S?=>)b9;DpoTXV(6ItkU`}8*m6tx_I{Solh_N#dfe{v="
BLOB_KEY = b"657f48f84c437cc1"

PAGE_SIZE = 4096
RESERVE = 80          # IV (16) + HMAC-SHA512 (64), reserved per-page tail
KDF_ITER = 256_000     # SQLCipher v4 default
HMAC_KDF_ITER = 2      # SQLCipher v4 default, HMAC-key derivation
HMAC_SALT_XOR = 0x3A
SALT_LEN = 16


def _deobfuscate_key() -> str:
    decoded = base64.b85decode(BLOB)
    xored = bytes(b ^ BLOB_KEY[i % len(BLOB_KEY)] for i, b in enumerate(decoded))
    return zlib.decompress(xored).decode("utf-8")


def _derive_keys(passphrase: str, salt: bytes) -> tuple[bytes, bytes]:
    prf = lambda p, s: HMAC.new(p, s, SHA512).digest()
    key = PBKDF2(passphrase.encode(), salt, dkLen=32, count=KDF_ITER, prf=prf)
    hmac_salt = bytes(b ^ HMAC_SALT_XOR for b in salt)
    hmac_key = PBKDF2(key, hmac_salt, dkLen=32, count=HMAC_KDF_ITER, prf=prf)
    return key, hmac_key


def decrypt_masterdb(raw: bytes) -> bytes:
    if len(raw) % PAGE_SIZE != 0:
        raise ValueError(f"truncated file: {len(raw)} bytes is not a multiple of {PAGE_SIZE}")

    passphrase = _deobfuscate_key()
    salt = raw[:SALT_LEN]
    key, hmac_key = _derive_keys(passphrase, salt)

    out = bytearray()
    for i in range(len(raw) // PAGE_SIZE):
        page_no = i + 1
        page = raw[i * PAGE_SIZE : (i + 1) * PAGE_SIZE]
        cstart = SALT_LEN if page_no == 1 else 0
        ciphertext = page[cstart : PAGE_SIZE - RESERVE]
        tail = page[PAGE_SIZE - RESERVE :]
        iv, stored_hmac = tail[:16], tail[16 : 16 + 64]

        mac = HMAC.new(hmac_key, digestmod=SHA512)
        mac.update(ciphertext)
        mac.update(iv)
        mac.update(page_no.to_bytes(4, "little"))
        mac.verify(stored_hmac)  # raises on mismatch — wrong key or corrupt page

        plain = bytearray(AES.new(key, AES.MODE_CBC, iv).decrypt(ciphertext))

        if page_no == 1:
            # The on-disk salt (not ciphertext) occupies what would be the
            # SQLite magic header string — it's never encrypted, so it has to
            # be reconstructed literally. Bytes 18/19 (write/read version) are
            # forced to 1/1 (rollback mode) — a real Rekordbox master.db reads
            # 2/2 (WAL) even after a clean shutdown, and nothing here ever
            # reopens a `-wal` sidecar. Byte 20 (reserved space per page) is
            # set to the true RESERVE so SQLite's own page-size math lines up.
            # Offsets below are relative to `plain` (post magic-prefix), same
            # convention as rekordbox_masterdb.rs's decrypt_masterdb.
            plain[2] = 1  # file offset 18: write_version
            plain[3] = 1  # file offset 19: read_version
            plain[4] = RESERVE  # file offset 20: reserved space per page
            out += b"SQLite format 3\x00"
        out += plain
        # Reassembled pages stay a fixed PAGE_SIZE — the trailing RESERVE
        # bytes are zero padding (the header at offset 20 declares the true
        # reserve; this padding is never real content).
        out += bytes(RESERVE)
    return bytes(out)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(1)
    src, dst = sys.argv[1], sys.argv[2]
    with open(src, "rb") as f:
        raw = f.read()
    plain = decrypt_masterdb(raw)
    with open(dst, "wb") as f:
        f.write(plain)
    print(f"decrypted {len(plain)} bytes -> {dst}")
