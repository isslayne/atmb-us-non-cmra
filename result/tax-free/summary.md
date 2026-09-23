# Address check: tax-free

- Checked: 67
- Smarty non-CMRA: 5
- Smarty errors: 0
- USPS errors/unavailable: 23
- Limited smoke test: false

Full per-address results, including USPS fields and raw JSON, are in `checks.csv`. USPS failures are not validation passes.

- Unavailable detail pages (validation skipped): 44

## Smarty statuses

| Status | Count |
| --- | --- |
| matched | 23 |
| skipped_detail_unavailable | 44 |

## USPS statuses

| Status | Count |
| --- | --- |
| http_302 | 3 |
| skipped_after_service_errors | 20 |
| skipped_detail_unavailable | 44 |
