# Chart specifications

`astraeus-specifications` defines reusable, versioned chart policy without
owning a person, session, timestamp, location, result, or persistence record.
Schema version 1 contains:

- validated object, zodiac, ayanamsa, and house-system calculation options;
- validated aspect definitions with explicit per-aspect orbs.

A specification combines with a validated UTC instant and geographic location
to produce an ordinary `CalculationRequest`. Calculation results and their
provenance continue to use `astraeus-artifacts`; artifact schema version 1 is
unchanged. This separation lets applications reuse named policies without
making names or application metadata part of calculation identity.

Unknown fields, unsupported schema versions, duplicate objects or aspects,
invalid zodiac/ayanamsa combinations, and invalid orbs fail during decoding.
