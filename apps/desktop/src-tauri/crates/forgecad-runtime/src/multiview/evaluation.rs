use serde_json::Value;

/// The metric order is intentionally shared by every orthographic view.  A
/// proposal that improves one scalar while regressing an earlier contour
/// metric is not eligible for promotion.
pub(crate) const METRIC_PRIORITY: [(&str, bool); 8] = [
    ("boundary_f1_4px", true),
    ("silhouette_iou", true),
    ("bbox_edge_error", false),
    ("centroid_error", false),
    ("landmark_coverage", true),
    ("landmark_nme", false),
    ("part_region_error", false),
    ("sdf_chamfer_px", false),
];

pub(crate) fn metric_non_regressing(baseline: &Value, proposal: &Value) -> (bool, bool) {
    let mut non_regressing = true;
    let mut strict = false;
    for (name, higher_is_better) in METRIC_PRIORITY {
        let Some(left) = baseline.get(name).and_then(Value::as_f64) else {
            return (false, false);
        };
        let Some(right) = proposal.get(name).and_then(Value::as_f64) else {
            return (false, false);
        };
        if !left.is_finite() || !right.is_finite() {
            return (false, false);
        }
        let improvement = if higher_is_better {
            right - left
        } else {
            left - right
        };
        if improvement < -1.0e-9 {
            non_regressing = false;
        }
        if improvement > 1.0e-9 {
            strict = true;
        }
    }
    (non_regressing, non_regressing && strict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(boundary: f64, iou: f64, bbox: f64, centroid: f64) -> Value {
        serde_json::json!({
            "boundary_f1_4px":boundary,
            "silhouette_iou":iou,
            "bbox_edge_error":bbox,
            "centroid_error":centroid,
            "landmark_coverage":1.0,
            "landmark_nme":0.1,
            "part_region_error":0.1,
            "sdf_chamfer_px":1.0
        })
    }

    #[test]
    fn earlier_contour_regression_blocks_even_if_later_metrics_improve() {
        let (non_regressing, strict) = metric_non_regressing(
            &metrics(0.80, 0.80, 0.20, 0.20),
            &metrics(0.79, 0.90, 0.10, 0.10),
        );
        assert!(!non_regressing);
        assert!(!strict);
    }

    #[test]
    fn a_strict_multi_metric_improvement_is_promotable() {
        let (non_regressing, strict) = metric_non_regressing(
            &metrics(0.80, 0.80, 0.20, 0.20),
            &metrics(0.82, 0.81, 0.18, 0.18),
        );
        assert!(non_regressing);
        assert!(strict);
    }
}
