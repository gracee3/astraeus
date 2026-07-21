# Astraeus

Astraeus is a validation-first Rust astrology and ephemeris engine.

The project is intentionally beginning with a clean history. The former
`gracee3/aphrodite-rust` repository remains the legacy source and provenance
record; code and fixtures will be imported only after review.

## Initial scope

- A small headless `astraeus-core` crate.
- Explicit calculation inputs, outputs, validation, and failure semantics.
- A provider boundary for Swiss Ephemeris and possible future alternatives.
- Deterministic golden tests against pinned `swetest` and Astrolog output.
- No GUI, HTTP service, database, Oracle Studio, tarot, or Magnolia dependency
  until the calculation foundation is independently validated.

## Track B handoff

Start with [the project organization and Track B handoff](docs/PROJECT_ORGANIZATION.md).
It records the repository boundaries, legacy sources, known defects, first
checkpoint, and non-goals.

The calculation contract lives in `astraeus-core`. The non-published
`astraeus-fixtures` crate verifies versioned external reference output without
adding a native ephemeris dependency. See [validation fixtures](docs/VALIDATION.md)
and the [Swiss Ephemeris integration policy](docs/SWISS_EPHEMERIS.md).

`astraeus-swiss` implements the provider contract with explicit Moshier and
Swiss-file modes. Swiss-file mode requires a caller-supplied data directory
and rejects silent fallback; no ephemeris data is bundled.

Every successful result includes validated [calculation provenance](docs/PROVENANCE.md)
covering its provider, runtime version, ephemeris source, and optional pinned
data revision.

`astraeus-artifacts` provides the versioned, content-addressed
[calculation artifact format](docs/ARTIFACTS.md) for safe hand-off to storage,
APIs, and future composition applications.

The core also provides deterministic [aspect detection](docs/ASPECTS.md) over
validated positions, with explicit per-aspect orbs and canonical pair ordering.

`astraeus-specifications` provides reusable schema-v1
[chart specifications](docs/CHART_SPECIFICATIONS.md) that combine calculation
choices and aspect policy without changing calculation artifact schema v1.

`astraeus-derived` combines a calculation artifact and matching specification
into a separately versioned, content-addressed
[derived chart artifact](docs/DERIVED_ARTIFACTS.md) with typed angles, derived
South Nodes, sign/house placements, and revalidated aspects.

`astraeus-western` adds separately versioned
[Western policy artifacts](docs/WESTERN_POLICIES.md) for traditional/modern
rulership, essential dignity, and selectable Chaldean/triplicity decans.

## License

AGPL-3.0-or-later. Swiss Ephemeris has its own dual-license requirements; its
adapter and distribution implications must be documented before integration.
