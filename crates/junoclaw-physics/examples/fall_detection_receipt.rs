//! Visual receipt: proves the safety envelope actually catches a violation,
//! and that the violation is cryptographically anchored via a Merkle proof.
//!
//! IMPORTANT HONESTY NOTE: `QuadrupedBackend` is a simplified analytical
//! model (tilt derived from a centripetal-force approximation), NOT a
//! rigid-body physics engine with contact-based collapse. It cannot
//! literally simulate a robot toppling over the way MuJoCo or real
//! hardware would. This example does NOT claim to show "a robot falling
//! down." It shows something narrower and fully true: an aggressive
//! maneuver pushes a sensor value (tilt) past a safety threshold, the
//! envelope check catches it every single cycle it happens, and the
//! resulting cycle hash is provably included in the batch's Merkle root.
//!
//! Run with:
//!   cargo run -p junoclaw-physics --example fall_detection_receipt
//!
//! Output:
//!   - `receipt_tilt_envelope.svg` — tilt-over-time chart with the safety
//!     threshold and violated cycles marked in red
//!   - Console "RECEIPT" block — batch merkle root, violation count, and
//!     a live Merkle proof verification for one violating cycle

use junoclaw_physics::{
    check_invariants, compute_merkle_proof, run_reflex_batch, verify_merkle_proof, BatchConfig,
    PhysicsSimulator, QuadrupedBackend, QuadrupedConfig,
};
use plotters::prelude::*;

fn main() {
    println!("=== JunoClaw Safety Envelope Receipt ===\n");
    println!("NOTE: simplified analytical sim, not rigid-body physics.");
    println!("This demonstrates threshold detection + Merkle proof, not a literal fall.\n");

    // Aggressive maneuver: maximal asymmetric hip torque across the trot
    // pairs drives propulsion + turn simultaneously, inducing the largest
    // centripetal tilt this simplified model can produce. This is a
    // stress test, not a collapse — the sim has no toppling dynamics.
    let mut sim = QuadrupedBackend::new("dogzilla-receipt-demo".to_string(), QuadrupedConfig::default());
    let max_t = QuadrupedConfig::default().max_joint_torque;
    let mut torques = [0.0; 15];
    torques[0] = max_t;   // fl_hip — propulsion
    torques[9] = max_t;   // rr_hip — propulsion
    torques[3] = max_t;   // fr_hip — turn (positive)
    torques[6] = -max_t;  // rl_hip — turn (negative, maximizes asymmetry)
    sim.set_joint_controls(&torques);

    let mut config = BatchConfig::quadruped_preset("dogzilla-receipt-demo");
    config.cycle_count = 500;

    let result = run_reflex_batch(&mut sim, &config);

    let tilts: Vec<f64> = result.states.iter().map(|s| s.sensors.tilt_degrees).collect();

    // Demo threshold: 50% of the peak tilt actually observed in this run.
    // Chosen after simulating (envelope checks are purely post-hoc and do
    // not affect the physics), so the receipt always shows a real,
    // non-cherry-picked crossing rather than a threshold tuned to always
    // pass or always fail.
    let peak_tilt = tilts.iter().cloned().fold(0.0f64, f64::max);
    let mut demo_envelope = config.envelope.clone();
    demo_envelope.max_tilt_degrees = peak_tilt * 0.5;

    let violated_cycles: Vec<usize> = result
        .states
        .iter()
        .enumerate()
        .filter(|(_, s)| !check_invariants(s, &demo_envelope).is_empty())
        .map(|(i, _)| i)
        .collect();

    // --- Chart ---
    let max_tilt = peak_tilt.max(demo_envelope.max_tilt_degrees) * 1.2;
    let root = SVGBackend::new("receipt_tilt_envelope.svg", (1000, 500)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Safety Envelope Receipt: Tilt vs. Threshold (simplified sim, not rigid-body physics)",
            ("sans-serif", 18),
        )
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(0..tilts.len(), 0.0..max_tilt)
        .unwrap();

    chart
        .configure_mesh()
        .x_desc("Reflex cycle")
        .y_desc("Tilt (degrees)")
        .draw()
        .unwrap();

    // Threshold line
    chart
        .draw_series(LineSeries::new(
            (0..tilts.len()).map(|i| (i, demo_envelope.max_tilt_degrees)),
            RED.mix(0.6).stroke_width(2),
        ))
        .unwrap()
        .label("Safety threshold")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));

    // Tilt trace
    chart
        .draw_series(LineSeries::new(
            (0..tilts.len()).map(|i| (i, tilts[i])),
            BLUE.stroke_width(2),
        ))
        .unwrap()
        .label("Measured tilt")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE));

    // Violated cycles marked
    chart
        .draw_series(
            violated_cycles
                .iter()
                .map(|&i| Circle::new((i, tilts[i]), 3, RED.filled())),
        )
        .unwrap()
        .label("Envelope violation")
        .legend(|(x, y)| Circle::new((x + 10, y), 3, RED.filled()));

    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()
        .unwrap();

    root.present().unwrap();

    // --- Merkle proof verification for one violating cycle ---
    let sample_idx = violated_cycles.first().copied().unwrap_or(0);
    let proof = compute_merkle_proof(&result.cycle_hashes, sample_idx);
    let recomputed_root = verify_merkle_proof(&result.cycle_hashes[sample_idx], sample_idx, &proof);
    let proof_valid = recomputed_root == result.attestation.merkle_root;

    let mut violated_invariant_names: Vec<String> = result
        .states
        .iter()
        .flat_map(|s| check_invariants(s, &demo_envelope))
        .collect();
    violated_invariant_names.sort();
    violated_invariant_names.dedup();

    // --- Console receipt ---
    println!("--- RECEIPT ---");
    println!("robot_id:              {}", result.attestation.robot_id);
    println!("cycle_count:           {}", result.attestation.cycle_count);
    println!("peak_tilt_observed:    {:.4} deg", peak_tilt);
    println!("demo_threshold (50%):  {:.4} deg", demo_envelope.max_tilt_degrees);
    println!("violations_detected:   {}", violated_cycles.len());
    println!("violated_invariants:   {:?}", violated_invariant_names);
    println!("batch_merkle_root:     {}", result.attestation.merkle_root);
    println!(
        "sample_cycle_hash[{}]:  {}",
        sample_idx, result.cycle_hashes[sample_idx]
    );
    println!("merkle_proof_len:      {}", proof.len());
    println!("merkle_proof_verified: {}", proof_valid);
    println!("chart_written_to:      receipt_tilt_envelope.svg");
    println!("\nAnyone can re-run this file, recompute the Merkle root from");
    println!("cycle_hashes, and confirm it matches batch_merkle_root above —");
    println!("without trusting this program's output.");
}
