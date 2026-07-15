from forgecad_agent.application.domain_inference import DomainInferenceService, infer_domain


def test_first_party_domain_is_recognized_without_provider_calls():
    result = infer_domain("设计一辆未来城市探索汽车，保留完整外观")
    assert result.status == "recognized"
    assert result.domain_pack_id == "pack_vehicle_concept"
    assert result.candidate_domain_pack_ids == ["pack_vehicle_concept"]
    assert result.matched_terms


def test_overlapping_brief_requires_clarification():
    result = infer_domain("设计一架可变形汽车飞机")
    assert result.status == "ambiguous"
    assert set(result.candidate_domain_pack_ids) == {
        "pack_vehicle_concept",
        "pack_aircraft_concept",
    }
    assert result.domain_pack_id is None


def test_unknown_brief_is_a_write_barrier():
    result = infer_domain("设计一个抽象的未来机械展示模型")
    assert result.status == "unsupported"
    assert result.candidate_domain_pack_ids == []
    assert result.matched_terms == []


def test_custom_service_keeps_candidates_unique():
    service = DomainInferenceService(
        {
            "pack_vehicle_concept": ("car",),
            "pack_aircraft_concept": ("aircraft",),
        }
    )
    result = service.infer("car aircraft")
    assert result.status == "ambiguous"
    assert len(result.candidate_domain_pack_ids) == 2
