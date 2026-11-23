"""A stand-in for Longshot's scripts/longshot_bridge.py so the unit tests run with no Longshot and no game. Only the names the
Mod UI server imports; nothing here talks to a game. Tests use the real file instead when LONGSHOT_SCRIPTS is set."""


class Failed(Exception):
    def __init__(self, message, code=1):
        super().__init__(message)
        self.code = code


class Bridge:
    def __init__(self, *a, **k):
        raise Failed("stub: no game", 2)


class Session:
    @classmethod
    def attach(cls, folder):
        raise Failed("stub: no game", 2)


def list_runs(root=None):
    return []


def load_flag_names(*a, **k):
    return {}


def read_flags(*a, **k):
    return {}


def murmur3(data, seed=0):
    if isinstance(data, str):
        data = data.encode()
    h = seed & 0xFFFFFFFF
    n = len(data) // 4 * 4
    for i in range(0, n, 4):
        k = int.from_bytes(data[i:i + 4], "little")
        k = (k * 0xCC9E2D51) & 0xFFFFFFFF
        k = ((k << 15) | (k >> 17)) & 0xFFFFFFFF
        k = (k * 0x1B873593) & 0xFFFFFFFF
        h ^= k
        h = ((h << 13) | (h >> 19)) & 0xFFFFFFFF
        h = (h * 5 + 0xE6546B64) & 0xFFFFFFFF
    k = 0
    tail = data[n:]
    for i in range(len(tail) - 1, -1, -1):
        k = (k << 8) | tail[i]
    if tail:
        k = (k * 0xCC9E2D51) & 0xFFFFFFFF
        k = ((k << 15) | (k >> 17)) & 0xFFFFFFFF
        k = (k * 0x1B873593) & 0xFFFFFFFF
        h ^= k
    h ^= len(data)
    h ^= h >> 16
    h = (h * 0x85EBCA6B) & 0xFFFFFFFF
    h ^= h >> 13
    h = (h * 0xC2B2AE35) & 0xFFFFFFFF
    h ^= h >> 16
    return h
