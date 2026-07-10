from __future__ import annotations

import sqlite3
from types import TracebackType
from typing import Optional, Type

from .connection import SQLiteConnectionFactory
from .concept_repositories import (
    ConceptAssetRepository,
    ChangeSetRepository,
    BriefVariantRepository,
    ConceptProjectRepository,
    DomainProfileRepository,
    ModuleRepository,
    QualityRepository,
)
from .repositories import AssetRepository, CheckpointRepository, IdempotencyRepository, JobRepository


class SQLiteUnitOfWork:
    """Own one SQLite transaction and its transaction-scoped repositories."""

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory
        self.connection: Optional[sqlite3.Connection] = None
        self.assets: AssetRepository
        self.checkpoints: CheckpointRepository
        self.idempotency: IdempotencyRepository
        self.jobs: JobRepository
        self.concept_projects: ConceptProjectRepository
        self.domain_profiles: DomainProfileRepository
        self.concept_assets: ConceptAssetRepository
        self.modules: ModuleRepository
        self.change_sets: ChangeSetRepository
        self.quality: QualityRepository
        self.brief_variants: BriefVariantRepository

    def __enter__(self) -> "SQLiteUnitOfWork":
        connection = self.connection_factory.connect()
        self.connection = connection
        self.assets = AssetRepository(connection)
        self.checkpoints = CheckpointRepository(connection)
        self.idempotency = IdempotencyRepository(connection)
        self.jobs = JobRepository(connection)
        self.concept_projects = ConceptProjectRepository(connection)
        self.domain_profiles = DomainProfileRepository(connection)
        self.concept_assets = ConceptAssetRepository(connection)
        self.modules = ModuleRepository(connection)
        self.change_sets = ChangeSetRepository(connection)
        self.quality = QualityRepository(connection)
        self.brief_variants = BriefVariantRepository(connection)
        return self

    def __exit__(
        self,
        exc_type: Optional[Type[BaseException]],
        exc_value: Optional[BaseException],
        traceback: Optional[TracebackType],
    ) -> None:
        if self.connection is None:
            return
        try:
            if exc_type is None:
                self.connection.commit()
            else:
                self.connection.rollback()
        finally:
            self.connection.close()
            self.connection = None

    def require_connection(self) -> sqlite3.Connection:
        if self.connection is None:
            raise RuntimeError("Unit of Work is not active.")
        return self.connection
