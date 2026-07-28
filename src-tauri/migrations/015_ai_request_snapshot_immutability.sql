CREATE TRIGGER trg_ai_request_snapshots_immutable_before_update
BEFORE UPDATE OF
  origin_collection_name_snapshot,
  origin_icon_name_snapshot,
  provider_mode,
  service_surface,
  provider,
  adapter_id,
  adapter_contract_version,
  account_context,
  model,
  operation,
  provenance_trust,
  credential_mode_snapshot,
  capability_snapshot_json,
  data_tier_snapshot_json,
  retention_snapshot_json,
  consent_snapshot_json,
  policy_refs_json,
  prompt_options_snapshot_json,
  input_package_sha256,
  mask_package_sha256,
  reference_package_sha256,
  original_lineage_id,
  original_lineage_generation,
  original_source_sha256,
  effective_source_sha256,
  payload_input_signature,
  request_recipe_signature,
  activation_revision,
  created_at
ON ai_requests
WHEN
  NEW.origin_collection_name_snapshot IS NOT OLD.origin_collection_name_snapshot
  OR NEW.origin_icon_name_snapshot IS NOT OLD.origin_icon_name_snapshot
  OR NEW.provider_mode IS NOT OLD.provider_mode
  OR NEW.service_surface IS NOT OLD.service_surface
  OR NEW.provider IS NOT OLD.provider
  OR NEW.adapter_id IS NOT OLD.adapter_id
  OR NEW.adapter_contract_version IS NOT OLD.adapter_contract_version
  OR NEW.account_context IS NOT OLD.account_context
  OR NEW.model IS NOT OLD.model
  OR NEW.operation IS NOT OLD.operation
  OR NEW.provenance_trust IS NOT OLD.provenance_trust
  OR NEW.credential_mode_snapshot IS NOT OLD.credential_mode_snapshot
  OR NEW.capability_snapshot_json IS NOT OLD.capability_snapshot_json
  OR NEW.data_tier_snapshot_json IS NOT OLD.data_tier_snapshot_json
  OR NEW.retention_snapshot_json IS NOT OLD.retention_snapshot_json
  OR NEW.consent_snapshot_json IS NOT OLD.consent_snapshot_json
  OR NEW.policy_refs_json IS NOT OLD.policy_refs_json
  OR NEW.prompt_options_snapshot_json IS NOT OLD.prompt_options_snapshot_json
  OR NEW.input_package_sha256 IS NOT OLD.input_package_sha256
  OR NEW.mask_package_sha256 IS NOT OLD.mask_package_sha256
  OR NEW.reference_package_sha256 IS NOT OLD.reference_package_sha256
  OR NEW.original_lineage_id IS NOT OLD.original_lineage_id
  OR NEW.original_lineage_generation IS NOT OLD.original_lineage_generation
  OR NEW.original_source_sha256 IS NOT OLD.original_source_sha256
  OR NEW.effective_source_sha256 IS NOT OLD.effective_source_sha256
  OR NEW.payload_input_signature IS NOT OLD.payload_input_signature
  OR NEW.request_recipe_signature IS NOT OLD.request_recipe_signature
  OR NEW.activation_revision IS NOT OLD.activation_revision
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'AI request provenance snapshots are immutable');
END;
