from __future__ import annotations

import sqlite3
from pathlib import Path


class SQLiteConnectionFactory:
    """Create consistently configured SQLite connections.

    Transaction ownership stays with the caller. This keeps repositories and
    Unit of Work code independent from the legacy AssetStore facade.
    """

    def __init__(self, db_path: Path, *, busy_timeout_ms: int = 5000) -> None:
        self.db_path = db_path.expanduser().resolve()
        self.busy_timeout_ms = busy_timeout_ms

    def connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.db_path)
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA foreign_keys=ON")
        connection.execute(f"PRAGMA busy_timeout={self.busy_timeout_ms}")
        return connection
