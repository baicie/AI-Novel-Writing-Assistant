-- P16.2B：并发控制与发现结果应用的版本列（纯增量）

ALTER TABLE provider_credential ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE provider_model ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;
