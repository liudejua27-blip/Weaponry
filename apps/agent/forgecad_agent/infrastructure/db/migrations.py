from __future__ import annotations

from pathlib import Path

from .connection import SQLiteConnectionFactory


class MigrationError(RuntimeError):
    pass


class SQLiteMigrationRunner:
    """Apply numbered SQL migrations exactly once."""

    def __init__(self, connection_factory: SQLiteConnectionFactory, migrations_dir: Path) -> None:
        self.connection_factory = connection_factory
        self.migrations_dir = migrations_dir.expanduser().resolve()

    def run(self) -> list[str]:
        migrations = sorted(self.migrations_dir.glob("*.sql"))
        if not migrations:
            raise MigrationError(f"No SQLite migrations found in {self.migrations_dir}")

        applied_now: list[str] = []
        with self.connection_factory.connect() as connection:
            has_migrations = connection.execute(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'"
            ).fetchone()
            if has_migrations is None:
                initial = self.migrations_dir / "0001_init.sql"
                if not initial.is_file():
                    raise MigrationError(f"Initial SQLite migration is missing: {initial}")
                connection.executescript(initial.read_text(encoding="utf-8"))
                applied_now.append("0001")

            applied = {
                str(row["version"])
                for row in connection.execute("SELECT version FROM schema_migrations").fetchall()
            }
            for migration in migrations:
                version = migration.stem.split("_", 1)[0]
                if version in applied:
                    continue
                connection.executescript(migration.read_text(encoding="utf-8"))
                applied.add(version)
                applied_now.append(version)
        return applied_now
