//! Physical Store boundary for the Knife surface pipeline.
//!
//! The formal High/Low/Cage/Bake link is one logical aggregate even though its
//! durable representation has a parent row and seven typed child rows.  This
//! module owns the aggregate's record projections and Store entry points.  It
//! deliberately borrows `Store`: the SQLite connection, migration sequence,
//! CAS registration and reachability transaction remain owned by `Store`.

use forgecad_contracts::{
    is_opaque_id, ProductionStageHeadV3Record, ProductionWeaponCageArtifactRecord,
    ProductionWeaponHighArtifactRecord, ProductionWeaponHighLowBakeGetResult,
    ProductionWeaponHighLowBakePlanRecord, ProductionWeaponHighLowBakePrepareResult,
    ProductionWeaponHighLowBakeReceiptRecord, ProductionWeaponHighLowCorrespondenceRecord,
    ProductionWeaponHighLowDiagnosticRecord, ProductionWeaponLowArtifactRecord,
    PRODUCTION_WEAPON_HIGH_LOW_GATE_SCOPES,
};

use super::{
    commit_production_weapon_high_low_bake_in_transaction,
    ensure_production_weapon_high_low_bake_bindings_in_transaction,
    prepare_production_weapon_high_low_bake, production_weapon_high_low_bake_get_result,
    production_weapon_high_low_bake_prepare_result, production_weapon_high_low_error,
    read_production_stage_head_v3_for_connection,
    read_production_weapon_high_low_bake_in_transaction,
    read_production_weapon_high_low_bake_preflight_sources, CasObjectRecord, Store, StoreError,
};

/// Read-only presence and hash summary for one immutable formal
/// High/Low/Cage/Bake link. This is deliberately a projection of SQLite
/// rows, not a formal Bake result and not a source-bundle alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionWeaponHighLowBakePreflightSourceSummary {
    pub bake_receipt_id: String,
    pub link_exists: bool,
    pub high_exists: bool,
    pub low_exists: bool,
    pub cage_exists: bool,
    pub correspondence_exists: bool,
    pub plan_exists: bool,
    pub diagnostic_exists: bool,
    pub receipt_exists: bool,
    /// All seven expected row roles are present. This is not typed/canonical
    /// formal-bake verification and must never be promoted to a quality pass.
    pub formal_rows_present: bool,
    pub canonical_sha256: String,
    pub receipt_object_sha256: String,
    pub cage_artifact_sha256: String,
    pub high_artifact_sha256: Option<String>,
    pub high_artifact_readback_object_sha256: Option<String>,
    pub low_artifact_sha256: Option<String>,
    pub low_artifact_readback_object_sha256: Option<String>,
    pub cage_artifact_readback_object_sha256: Option<String>,
    pub correspondence_object_sha256: Option<String>,
    pub bake_plan_object_sha256: Option<String>,
    pub diagnostic_object_sha256: Option<String>,
}

/// Store-only read result for the exact project/session/root-candidate key.
/// `head` is the current durable `ProductionStageHeadV3` projection; formal
/// rows are reported independently and are never synthesized from a source
/// bundle or from a candidate-surface bake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionWeaponHighLowBakePreflightSources {
    pub head: Option<ProductionStageHeadV3Record>,
    pub formal_bake_links: Vec<ProductionWeaponHighLowBakePreflightSourceSummary>,
}

/// All immutable inputs for one formal High/Low/Cage/Bake Store commit.
/// Runtime materializes the seven typed records and registers their CAS
/// objects before handing this closed bundle to the Store.  `owned_objects`
/// must contain every CAS root referenced by those records; the Store marks
/// the complete set reachable in the same SQLite transaction as the seven
/// child rows.
#[derive(Debug, Clone)]
pub struct ProductionWeaponHighLowBakeCommitBundle {
    pub high: ProductionWeaponHighArtifactRecord,
    pub low: ProductionWeaponLowArtifactRecord,
    pub cage: ProductionWeaponCageArtifactRecord,
    pub correspondence: ProductionWeaponHighLowCorrespondenceRecord,
    pub plan: ProductionWeaponHighLowBakePlanRecord,
    pub diagnostic: ProductionWeaponHighLowDiagnosticRecord,
    pub receipt: ProductionWeaponHighLowBakeReceiptRecord,
    pub owned_objects: Vec<CasObjectRecord>,
}

/// Borrowed repository façade for the formal surface aggregate.
///
/// This is an API seam for Runtime services; it cannot create a second Store,
/// SQLite connection, migration owner or CAS root policy.  The compatibility
/// methods on `Store` below remain public and retain their exact signatures.
#[derive(Clone, Copy)]
pub struct SurfaceRepository<'store> {
    store: &'store Store,
}

impl<'store> SurfaceRepository<'store> {
    pub(crate) fn new(store: &'store Store) -> Self {
        Self { store }
    }

    /// Read the current V3 head and exact formal High/Low/Cage/Bake rows for
    /// one project/session/root-candidate key.
    pub fn get_production_weapon_high_low_bake_preflight_sources(
        &self,
        project_id: &str,
        session_id: &str,
        candidate_id: &str,
    ) -> Result<ProductionWeaponHighLowBakePreflightSources, StoreError> {
        self.store
            .get_production_weapon_high_low_bake_preflight_sources(
                project_id,
                session_id,
                candidate_id,
            )
    }

    /// Atomically persist the formal High/Low/Cage/Bake aggregate and its
    /// seven typed child rows.
    pub fn commit_production_weapon_high_low_bake(
        &self,
        bundle: &ProductionWeaponHighLowBakeCommitBundle,
    ) -> Result<ProductionWeaponHighLowBakePrepareResult, StoreError> {
        self.store.commit_production_weapon_high_low_bake(bundle)
    }

    /// Read and re-verify one exact formal High/Low/Cage/Bake receipt after a
    /// Store/CAS restart.
    pub fn get_production_weapon_high_low_bake(
        &self,
        project_id: &str,
        session_id: &str,
        bake_receipt_id: &str,
        gate_scope: &str,
    ) -> Result<Option<ProductionWeaponHighLowBakeGetResult>, StoreError> {
        self.store.get_production_weapon_high_low_bake(
            project_id,
            session_id,
            bake_receipt_id,
            gate_scope,
        )
    }
}

impl Store {
    /// Borrow the physical formal High/Low/Cage/Bake repository.
    ///
    /// This constructor is side-effect free. `Store::migrate` remains the
    /// single migration owner, and all aggregate writes continue to use the
    /// Store-owned connection and CAS reachability transaction.
    pub fn surface_repository(&self) -> SurfaceRepository<'_> {
        SurfaceRepository::new(self)
    }

    /// Read the current V3 head and the exact formal High/Low/Cage/Bake rows
    /// for one project/session/root-candidate key. This method is deliberately
    /// Store-only: it does not write SQLite or CAS, mark objects reachable,
    /// invoke a Worker, or reinterpret the source-only retopology/cage bundle
    /// as a formal Bake result.
    pub fn get_production_weapon_high_low_bake_preflight_sources(
        &self,
        project_id: &str,
        session_id: &str,
        candidate_id: &str,
    ) -> Result<ProductionWeaponHighLowBakePreflightSources, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(session_id) || !is_opaque_id(candidate_id) {
            return Err(StoreError::InvalidData(
                "ProductionWeaponHighLowBake preflight binding is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let head = read_production_stage_head_v3_for_connection(
            &mut connection,
            &self.cas,
            session_id,
            project_id,
            candidate_id,
        )?;
        let formal_bake_links = read_production_weapon_high_low_bake_preflight_sources(
            &connection,
            &self.cas,
            head.as_ref(),
            project_id,
            session_id,
            candidate_id,
        )?;
        Ok(ProductionWeaponHighLowBakePreflightSources {
            head,
            formal_bake_links,
        })
    }

    /// Atomically persist the formal High/Low/Cage/Bake link and all seven
    /// typed child rows.  The operation is idempotent on `bake_receipt_id`:
    /// an exact replay returns the durable receipt with `replayed = true`,
    /// while a retargeted payload is rejected without leaving partial rows or
    /// promoted CAS roots behind.
    pub fn commit_production_weapon_high_low_bake(
        &self,
        bundle: &ProductionWeaponHighLowBakeCommitBundle,
    ) -> Result<ProductionWeaponHighLowBakePrepareResult, StoreError> {
        let prepared = prepare_production_weapon_high_low_bake(&self.cas, bundle)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let (receipt, replayed) = commit_production_weapon_high_low_bake_in_transaction(
            &transaction,
            &self.cas,
            &prepared,
        )?;
        transaction.commit()?;
        Ok(production_weapon_high_low_bake_prepare_result(
            receipt, replayed,
        ))
    }

    /// Read and re-verify one exact formal High/Low/Cage/Bake receipt after a
    /// Store/CAS restart.  The project/session/receipt/scope tuple is the
    /// complete lookup key; no caller-supplied replacement payload can alter
    /// the returned receipt object hash.
    pub fn get_production_weapon_high_low_bake(
        &self,
        project_id: &str,
        session_id: &str,
        bake_receipt_id: &str,
        gate_scope: &str,
    ) -> Result<Option<ProductionWeaponHighLowBakeGetResult>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(session_id) || !is_opaque_id(bake_receipt_id)
        {
            return Err(StoreError::InvalidData(
                "ProductionWeaponHighLowBake lookup identity is invalid".to_owned(),
            ));
        }
        if !PRODUCTION_WEAPON_HIGH_LOW_GATE_SCOPES.contains(&gate_scope) {
            return Err(production_weapon_high_low_error(
                "PRODUCTION_WEAPON_HIGH_LOW_GATE_SCOPE_INVALID",
                "unknown bake gate scope",
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let Some(prepared) = read_production_weapon_high_low_bake_in_transaction(
            &transaction,
            &self.cas,
            project_id,
            session_id,
            bake_receipt_id,
            Some(gate_scope),
        )?
        else {
            transaction.rollback()?;
            return Ok(None);
        };
        if prepared
            .owned_objects
            .iter()
            .any(|object| object.reachability != "reachable")
        {
            return Err(production_weapon_high_low_error(
                "PRODUCTION_WEAPON_HIGH_LOW_REACHABILITY_MISMATCH",
                "formal bake readback contains a temporary or quarantined owned CAS root",
            ));
        }
        ensure_production_weapon_high_low_bake_bindings_in_transaction(
            &transaction,
            &prepared,
            false,
        )?;
        let result = production_weapon_high_low_bake_get_result(&prepared.receipt);
        transaction.commit()?;
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_surface_repository_uses_the_store_connection_and_zero_write_preflight() {
        let store = Store::memory().expect("store");
        let repository = store.surface_repository();
        assert!(std::ptr::eq(repository.store, &store));

        let preflight = repository
            .get_production_weapon_high_low_bake_preflight_sources(
                "weaponry",
                "session-missing",
                "candidate-missing",
            )
            .expect("preflight");
        assert!(preflight.head.is_none());
        assert!(preflight.formal_bake_links.is_empty());
        assert_eq!(
            store
                .get_production_weapon_high_low_bake(
                    "weaponry",
                    "session-missing",
                    "bake-missing",
                    "high-low-bake",
                )
                .expect("missing aggregate"),
            None
        );
    }

    #[test]
    fn surface_repository_preserves_invalid_lookup_validation_before_store_mutation() {
        let store = Store::memory().expect("store");
        let repository = store.surface_repository();
        let error = repository
            .get_production_weapon_high_low_bake("", "session", "receipt", "high-low-bake")
            .expect_err("invalid project id");
        assert!(matches!(error, StoreError::InvalidData(_)));
    }
}
