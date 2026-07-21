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

## License

AGPL-3.0-or-later. Swiss Ephemeris has its own dual-license requirements; its
adapter and distribution implications must be documented before integration.
