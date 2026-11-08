# Data provenance and licensing

Bangla.gov.bd resources (EBLICT/BCC): BDNC, `banglagov` Hugging Face models,
Digital Lexicon etc. were reviewed. Their `/api` endpoints require
authentication and no public bulk download is available from them today, so
nothing from bangla.gov.bd is vendored here. Contact pdeblict@bcc.net.bd /
`/bn/service-request` if licensing for research use needs to be cleared.

- `openbangla-keyboard/data/dictionary.json` — submodule, upstream license.
- `frequency.json` — Leipzig Corpora Collection (CC BY 4.0); see
  `frequency.LICENSE` for attribution.
- `avro-patterns.json`, `khipro-patterns.json`, `common-patterns.txt` —
  generated tables from the OpenBangla organization repositories
  (`upodesh` main/khipro) derived from okkhor/riti rules; internal use only.
- `khipro-testcases.tsv` — KhiproTeam Khipro layout test corpus
  (used as regression vectors only).
