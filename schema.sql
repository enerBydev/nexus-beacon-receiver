-- D1 Database Schema for nexus-beacon-receiver
-- Two tables: beacons (per-instance daily data) and daily_global_stats (aggregated daily stats)

CREATE TABLE IF NOT EXISTS beacons (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_id TEXT NOT NULL,
    version TEXT NOT NULL,
    date TEXT NOT NULL,
    total_requests INTEGER NOT NULL DEFAULT 0,
    unique_fingerprints INTEGER NOT NULL DEFAULT 0,
    models_used TEXT NOT NULL DEFAULT '{}',
    client_types TEXT NOT NULL DEFAULT '{}',
    avg_message_count REAL NOT NULL DEFAULT 0.0,
    tool_use_ratio REAL NOT NULL DEFAULT 0.0,
    received_at TEXT NOT NULL DEFAULT (DATETIME('now')),
    UNIQUE(instance_id, date)
);

CREATE TABLE IF NOT EXISTS daily_global_stats (
    date TEXT PRIMARY KEY,
    total_instances INTEGER NOT NULL DEFAULT 0,
    total_requests INTEGER NOT NULL DEFAULT 0,
    total_unique_users INTEGER NOT NULL DEFAULT 0,
    models_used TEXT NOT NULL DEFAULT '{}',
    client_types TEXT NOT NULL DEFAULT '{}',
    avg_message_count REAL NOT NULL DEFAULT 0.0,
    tool_use_ratio REAL NOT NULL DEFAULT 0.0,
    versions TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (DATETIME('now'))
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_beacons_date ON beacons(date);
CREATE INDEX IF NOT EXISTS idx_beacons_instance ON beacons(instance_id);
CREATE INDEX IF NOT EXISTS idx_beacons_instance_date ON beacons(instance_id, date);
