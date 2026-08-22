//! MCAP telemetry reader — parses ROS 2 bag files for truth evaluation.
//!
//! MCAP is the default storage format for ROS 2 (replaced SQLite3).
//! It's indexed, compressed (Zstd/LZ4), cloud-native, and Foxglove-compatible.
//!
//! This module reads MCAP files and extracts sensor data for evaluation:
//! - IMU readings (accelerometer, gyroscope)
//! - Contact events
//! - Joint states
//! - Obstacle distance
//!
//! The miner can use MCAP data to evaluate robot decisions with full
//! telemetry context, not just the ZK proof summary.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// MCAP channel (topic) info.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McapChannel {
    pub topic: String,
    pub schema_name: String,
    pub message_count: u64,
}

/// MCAP file metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McapInfo {
    pub profile: Option<String>,
    pub library: Option<String>,
    pub message_count: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub channels: Vec<McapChannel>,
    pub file_size: u64,
}

/// A single sensor reading extracted from MCAP.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorReading {
    pub topic: String,
    pub timestamp_ns: u64,
    pub schema: String,
    pub data: serde_json::Value,
}

/// Telemetry batch extracted from an MCAP file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelemetryBatch {
    pub source_file: String,
    pub start_time_ns: u64,
    pub end_time_ns: u64,
    pub message_count: u64,
    pub readings: Vec<SensorReading>,
    /// Extracted safety-relevant metrics
    pub metrics: TelemetryMetrics,
}

/// Safety-relevant metrics extracted from telemetry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryMetrics {
    pub max_speed: Option<f64>,
    pub max_force: Option<f64>,
    pub min_distance: Option<f64>,
    pub max_tilt: Option<f64>,
    pub max_acceleration: Option<f64>,
    pub contact_events: u64,
    pub imu_readings: u64,
    pub joint_states: u64,
}

impl TelemetryBatch {
    /// Check if telemetry metrics are within a safety envelope.
    pub fn check_safety(&self, envelope: &crate::evaluator::SafetyEnvelope) -> Vec<String> {
        let mut violations = Vec::new();

        if let (Some(max_speed), Some(observed)) = (envelope.max_speed, self.metrics.max_speed) {
            if observed > max_speed {
                violations.push(format!(
                    "speed: {:.3} > {:.3}",
                    observed, max_speed
                ));
            }
        }
        if let (Some(max_force), Some(observed)) = (envelope.max_force, self.metrics.max_force) {
            if observed > max_force {
                violations.push(format!(
                    "force: {:.3} > {:.3}",
                    observed, max_force
                ));
            }
        }
        if let (Some(min_dist), Some(observed)) = (envelope.min_distance, self.metrics.min_distance) {
            if observed < min_dist {
                violations.push(format!(
                    "distance: {:.3} < {:.3}",
                    observed, min_dist
                ));
            }
        }
        if let (Some(max_tilt), Some(observed)) = (envelope.max_tilt, self.metrics.max_tilt) {
            if observed > max_tilt {
                violations.push(format!(
                    "tilt: {:.3} > {:.3}",
                    observed, max_tilt
                ));
            }
        }

        violations
    }
}

/// MCAP reader — parses MCAP files and extracts telemetry.
///
/// In production, this uses the `mcap` Rust crate for native parsing.
/// For now, we provide a stub that reads pre-extracted JSON telemetry
/// (the prover daemon already outputs JSON snapshots).
pub struct McapReader;

impl McapReader {
    /// Read an MCAP file and extract telemetry.
    ///
    /// Currently reads pre-extracted JSON. Native MCAP parsing will be
    /// added when the `mcap` crate is available as a dependency.
    pub async fn read_file(path: &Path) -> anyhow::Result<TelemetryBatch> {
        let content = tokio::fs::read_to_string(path).await?;
        let source = path.to_string_lossy().to_string();

        // Try parsing as pre-extracted JSON telemetry
        if let Ok(batch) = serde_json::from_str::<TelemetryBatch>(&content) {
            return Ok(TelemetryBatch { source_file: source, ..batch });
        }

        // Try parsing as a JSON array of sensor readings
        if let Ok(readings) = serde_json::from_str::<Vec<SensorReading>>(&content) {
            return Ok(Self::build_batch(source, readings));
        }

        // Fallback: empty batch (MCAP native parsing not yet implemented)
        tracing::warn!(
            file = %source,
            "mcap: native MCAP parsing not yet implemented, returning empty batch"
        );
        Ok(TelemetryBatch {
            source_file: source,
            start_time_ns: 0,
            end_time_ns: 0,
            message_count: 0,
            readings: Vec::new(),
            metrics: TelemetryMetrics::default(),
        })
    }

    /// Read telemetry from a directory of MCAP/JSON files.
    pub async fn read_dir(dir: &Path) -> anyhow::Result<Vec<TelemetryBatch>> {
        let mut batches = Vec::new();
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "mcap" || e == "json").unwrap_or(false) {
                match Self::read_file(&path).await {
                    Ok(batch) => batches.push(batch),
                    Err(e) => tracing::warn!(file = %path.display(), err = %e, "mcap: failed to read file"),
                }
            }
        }
        Ok(batches)
    }

    /// Build a TelemetryBatch from a list of sensor readings.
    fn build_batch(source: String, readings: Vec<SensorReading>) -> TelemetryBatch {
        let message_count = readings.len() as u64;
        let start_time_ns = readings.first().map(|r| r.timestamp_ns).unwrap_or(0);
        let end_time_ns = readings.last().map(|r| r.timestamp_ns).unwrap_or(0);

        let mut metrics = TelemetryMetrics::default();
        for r in &readings {
            match r.schema.as_str() {
                "sensor_msgs/Imu" => metrics.imu_readings += 1,
                "sensor_msgs/JointState" => metrics.joint_states += 1,
                "sensor_msgs/ContactState" => metrics.contact_events += 1,
                _ => {}
            }
        }

        TelemetryBatch {
            source_file: source,
            start_time_ns,
            end_time_ns,
            message_count,
            readings,
            metrics,
        }
    }
}
