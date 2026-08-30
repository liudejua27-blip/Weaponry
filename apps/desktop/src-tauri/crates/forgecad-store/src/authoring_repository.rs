//! Physical Store boundary for the Knife authoring slice.
//!
//! `AuthoringRepository` is intentionally a borrowed view over an existing
//! [`Store`].  It owns no connection, CAS root, migration sequence, or
//! reachability policy.  The existing Store methods remain the compatibility
//! implementation for now; this type is the single typed entry point that a
//! new Runtime authoring service can depend on while those implementations
//! are moved behind the boundary in later extraction atoms.
//!
//! Keeping the repository borrowed is important: creating a repository must
//! not create a second SQLite connection/database or a second migration
//! authority.  All writes and reads below therefore route through the same
//! Store transaction and CAS validation paths used by the compatibility API.

use super::{
    AuthoringMeshV2TransactionCommit, AuthoringMeshV2TransactionDurableRecord,
    KnifeCurveEvaluatedMeshCommit, KnifeCurveEvaluatedMeshDurableRecord, Store, StoreError,
    WeaponryCurveModifierGraphCommit, WeaponryCurveModifierGraphDurableRecord,
};
use serde_json::Value;

/// The first physically extracted Store repository.
///
/// The lifetime documents the ownership rule in the type system: the
/// repository cannot outlive the Store whose connection, CAS and migration
/// coordinator it uses.  No `Store::clone` or independent `Connection` is
/// performed here.
#[derive(Clone, Copy)]
pub struct AuthoringRepository<'store> {
    store: &'store Store,
}

impl<'store> AuthoringRepository<'store> {
    pub(crate) fn new(store: &'store Store) -> Self {
        Self { store }
    }

    // AuthoringMesh transaction -------------------------------------------------

    pub fn record_authoring_mesh_transaction_with_replay(
        &self,
        commit: &AuthoringMeshV2TransactionCommit,
    ) -> Result<(AuthoringMeshV2TransactionDurableRecord, bool), StoreError> {
        self.store
            .record_authoring_mesh_v2_transaction_with_replay(commit)
    }

    pub fn get_authoring_mesh_transaction(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<AuthoringMeshV2TransactionDurableRecord>, StoreError> {
        self.store
            .get_authoring_mesh_v2_transaction(project_id, idempotency_key)
    }

    pub fn get_authoring_mesh_transaction_by_id(
        &self,
        project_id: &str,
        transaction_id: &str,
    ) -> Result<Option<AuthoringMeshV2TransactionDurableRecord>, StoreError> {
        self.store
            .get_authoring_mesh_v2_transaction_by_id(project_id, transaction_id)
    }

    // Knife Curve / ModifierGraph ----------------------------------------------

    pub fn read_knife_curve_modifier_graph_json(
        &self,
        sha256: &str,
        expected_kind: &str,
    ) -> Result<Value, StoreError> {
        self.store
            .read_weaponry_curve_modifier_graph_json(sha256, expected_kind)
    }

    pub fn record_knife_curve_modifier_graph_with_replay(
        &self,
        commit: &WeaponryCurveModifierGraphCommit,
    ) -> Result<(WeaponryCurveModifierGraphDurableRecord, bool), StoreError> {
        self.store
            .record_weaponry_curve_modifier_graph_with_replay(commit)
    }

    pub fn get_knife_curve_modifier_graph(
        &self,
        project_id: &str,
        lookup_key_sha256: &str,
    ) -> Result<Option<WeaponryCurveModifierGraphDurableRecord>, StoreError> {
        self.store
            .get_weaponry_curve_modifier_graph(project_id, lookup_key_sha256)
    }

    pub fn get_knife_curve_modifier_graph_by_source_revision_and_modifier_graph(
        &self,
        project_id: &str,
        source_revision_sha256: &str,
        modifier_graph_sha256: &str,
        curve_set_sha256: &str,
        sample_set_sha256: &str,
        dependency_graph_sha256: &str,
        recompute_plan_sha256: &str,
    ) -> Result<Option<WeaponryCurveModifierGraphDurableRecord>, StoreError> {
        self.store
            .get_weaponry_curve_modifier_graph_by_source_revision_and_modifier_graph(
                project_id,
                source_revision_sha256,
                modifier_graph_sha256,
                curve_set_sha256,
                sample_set_sha256,
                dependency_graph_sha256,
                recompute_plan_sha256,
            )
    }

    pub fn knife_curve_modifier_graph_cas_roots(
        &self,
        record: &WeaponryCurveModifierGraphDurableRecord,
    ) -> Vec<String> {
        Store::weaponry_curve_modifier_graph_cas_roots(record)
    }

    // Knife Curve / EvaluatedMesh -----------------------------------------------

    pub fn read_knife_curve_evaluated_mesh_json(
        &self,
        sha256: &str,
        expected_kind: &str,
    ) -> Result<Value, StoreError> {
        self.store
            .read_weaponry_curve_evaluated_mesh_json(sha256, expected_kind)
    }

    pub fn record_knife_curve_evaluated_mesh_with_replay(
        &self,
        commit: &KnifeCurveEvaluatedMeshCommit,
    ) -> Result<(KnifeCurveEvaluatedMeshDurableRecord, bool), StoreError> {
        self.store
            .record_knife_curve_evaluated_mesh_with_replay(commit)
    }

    pub fn get_knife_curve_evaluated_mesh(
        &self,
        project_id: &str,
        evaluated_mesh_lookup_key_sha256: &str,
    ) -> Result<Option<KnifeCurveEvaluatedMeshDurableRecord>, StoreError> {
        self.store
            .get_knife_curve_evaluated_mesh(project_id, evaluated_mesh_lookup_key_sha256)
    }

    pub fn knife_curve_evaluated_mesh_cas_roots(
        &self,
        record: &KnifeCurveEvaluatedMeshDurableRecord,
    ) -> Vec<String> {
        Store::weaponry_curve_evaluated_mesh_cas_roots(record)
    }
}

impl Store {
    /// Borrow the first physical authoring repository from this Store.
    ///
    /// This constructor is deliberately side-effect free: it does not open a
    /// connection, run migrations, or create a CAS root.  `Store::migrate`
    /// remains the sole migration coordinator.
    pub fn authoring_repository(&self) -> AuthoringRepository<'_> {
        AuthoringRepository::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CasObjectRecord, ProjectRecord, WeaponryCurveModifierGraphCasBundle};
    use forgecad_core::{canonical_json_bytes, canonical_json_hash};
    use serde_json::json;

    fn hash(seed: &str) -> String {
        canonical_json_hash(&json!({"seed": seed}))
    }

    fn project(store: &Store) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: "weaponry".to_owned(),
                name: "Weaponry repository test".to_owned(),
                policy: json!({"scope":"test"}),
                created_at: "1".to_owned(),
                updated_at: "1".to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: "a".repeat(64),
            })
            .expect("project");
    }

    fn object(store: &Store, kind: &str, name: &str) -> CasObjectRecord {
        let bytes =
            canonical_json_bytes(&json!({"kind":kind,"name":name})).expect("canonical object");
        store
            .put_object(&bytes, None, "application/json", kind, "1")
            .expect("CAS object")
            .record
    }

    fn modifier_graph_commit(store: &Store) -> WeaponryCurveModifierGraphCommit {
        let cas = WeaponryCurveModifierGraphCasBundle {
            curve_set: object(store, "weaponry-curve-set", "curve"),
            sample_set: object(store, "weaponry-curve-sample-set", "sample"),
            modifier_graph: object(store, "weaponry-modifier-graph", "graph"),
            dependency_graph: object(store, "weaponry-dependency-graph", "dependency"),
            recompute_plan: object(store, "weaponry-recompute-plan", "recompute"),
        };
        let mut record = WeaponryCurveModifierGraphDurableRecord {
            schema_version: "WeaponryCurveModifierGraphDurableRecord@1".to_owned(),
            project_id: "weaponry".to_owned(),
            source_revision_id: "source-r1".to_owned(),
            source_revision_sha256: hash("source"),
            source_candidate_id: "candidate-r1".to_owned(),
            source_candidate_state_sha256: hash("candidate-state"),
            source_authoring_mesh_id: "mesh-r1".to_owned(),
            source_authoring_mesh_lineage_id: "lineage-r1".to_owned(),
            source_authoring_mesh_revision_index: 1,
            source_authoring_mesh_identity_sha256: hash("mesh-identity"),
            curve_set_id: "curve-set".to_owned(),
            curve_set_sha256: hash("curve"),
            curve_set_object_sha256: cas.curve_set.sha256.clone(),
            sample_set_id: "sample-set".to_owned(),
            sample_set_sha256: hash("sample"),
            sample_set_object_sha256: cas.sample_set.sha256.clone(),
            modifier_graph_id: "graph".to_owned(),
            modifier_graph_sha256: hash("graph"),
            modifier_graph_object_sha256: cas.modifier_graph.sha256.clone(),
            dependency_graph_sha256: hash("dependency"),
            dependency_graph_object_sha256: cas.dependency_graph.sha256.clone(),
            recompute_plan_sha256: hash("recompute"),
            recompute_plan_object_sha256: cas.recompute_plan.sha256.clone(),
            lookup_key_sha256: hash("lookup"),
            idempotency_key: "idem-1".to_owned(),
            input_sha256: hash("input"),
            materialization_status: "runtime-owned-store-weaponry-curve-modifier-graph@1"
                .to_owned(),
            canonical_sha256: String::new(),
            created_at: "1".to_owned(),
        };
        let mut value = serde_json::to_value(&record).expect("record value");
        value["canonical_sha256"] = Value::String(String::new());
        record.canonical_sha256 = canonical_json_hash(&value);
        WeaponryCurveModifierGraphCommit { record, cas }
    }

    #[test]
    fn repository_is_a_borrowed_store_boundary_and_real_curve_write_path() {
        let store = Store::memory().expect("store");
        project(&store);
        let repository = store.authoring_repository();
        assert!(std::ptr::eq(repository.store, &store));

        let commit = modifier_graph_commit(&store);
        let (stored, replayed) = repository
            .record_knife_curve_modifier_graph_with_replay(&commit)
            .expect("repository write");
        assert!(!replayed);
        assert_eq!(stored, commit.record);

        // Read through both surfaces: the repository call really installed a
        // row in the Store-owned SQLite connection rather than returning a
        // static directory result.
        assert_eq!(
            repository
                .get_knife_curve_modifier_graph("weaponry", &commit.record.lookup_key_sha256)
                .expect("repository read"),
            Some(commit.record.clone())
        );
        assert_eq!(
            store
                .get_weaponry_curve_modifier_graph("weaponry", &commit.record.lookup_key_sha256)
                .expect("compatibility read"),
            Some(commit.record.clone())
        );
        for root in repository.knife_curve_modifier_graph_cas_roots(&commit.record) {
            assert_eq!(
                store
                    .get_object(&root)
                    .expect("CAS metadata")
                    .expect("root")
                    .reachability,
                "reachable"
            );
        }

        let (replayed_record, was_replay) = repository
            .record_knife_curve_modifier_graph_with_replay(&commit)
            .expect("repository replay");
        assert!(was_replay);
        assert_eq!(replayed_record, commit.record);
    }

    #[test]
    fn repository_unifies_transaction_and_evaluated_mesh_lookups_without_second_store() {
        let store = Store::memory().expect("store");
        let repository = store.authoring_repository();
        let missing_hash = hash("missing");

        // These lookups use the real Store transaction path and remain
        // side-effect free when no durable row exists.
        assert_eq!(
            repository
                .get_authoring_mesh_transaction("weaponry", "transaction-missing")
                .expect("transaction lookup"),
            None
        );
        assert_eq!(
            repository
                .get_authoring_mesh_transaction_by_id("weaponry", "transaction-missing")
                .expect("transaction id lookup"),
            None
        );
        assert_eq!(
            repository
                .get_knife_curve_evaluated_mesh("weaponry", &missing_hash)
                .expect("evaluated mesh lookup"),
            None
        );
        assert!(
            repository
                .store
                .get_project("weaponry")
                .expect("same store")
                .is_none()
        );
    }
}
