# P14.9 Performance

Command: `cargo test -p novel-application --test p13_scale_baseline -- --nocapture`

| Chapters | Build ms | Context P50/P95 us | FTS P50/P95 us | DB bytes | Revisions | Chunks |
| --- | --- | --- | --- | --- | --- | --- |
| 100 | 284 | 4581 / 4837 | 1324 / 2568 | 5,443,584 | 100 | 99 |
| 500 | 1549 | 13144 / 13562 | 9718 / 12622 | 25,247,744 | 500 | 499 |
| 1000 | 2969 | 24703 / 26091 | 19005 / 23853 | 49,942,528 | 1000 | 999 |

Synthetic Mock/SQLite data, single run. No SLA claimed. App cold start, Task Center/Batch P95, export timing, memory/CPU peak were not measured in this table.
