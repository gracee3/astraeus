# Validation fixtures

The golden fixture harness is deliberately separate from the calculation
engine. `astraeus-core` owns validated domain values and complete results;
`astraeus-fixtures` owns versioned external evidence, parsing, tolerances, and
comparison reports.

## Baseline semantics

The first fixtures describe apparent, geocentric ecliptic-of-date positions at
`2000-01-01T12:00:00Z`. They use Greenwich Observatory coordinates (latitude
51.4779°, longitude 0°, elevation metadata 46 m), Placidus houses, decimal
degrees, astronomical-unit distances, and longitude speed in degrees per day.
The sidereal fixture selects Lahiri (`swetest -sid1`).

The expanded matrix adds `2024-04-08T18:00:00Z` at New York City coordinates
(latitude 40.7128°, longitude -74.006°, elevation metadata 10 m), Koch houses,
and both tropical and Fagan–Bradley sidereal (`swetest -sid0`) calculations.
This deliberately changes date, location, house algorithm, and ayanamsa while
retaining the same object set and numerical tolerances.

These fixtures explicitly use the built-in Moshier engine (`-emos`). They prove
the input, output, sidereal, speed, house, provenance, and tolerance contracts
without requiring Swiss `.se1` data files. They do not validate a future native
adapter's Swiss-file mode. That adapter must add separate `-eswe` fixtures and
must report missing data rather than silently fall back to Moshier.

Chiron is excluded because the selected Moshier baseline cannot calculate it
without external asteroid data. Sun through Pluto plus mean and true nodes are
included.

## Pinned source

- Repository: `https://github.com/aloistr/swisseph`
- Tag: `v2.10.03`
- Commit: `175e1fcb3108bcd5c0d146c803f51dcf23508012`
- Source archive SHA-256:
  `4a954f706c1eb7d2c5ead03d9f7e721820579ace2003fa8d438809534670786a`

The newer `v2.10.3final` tag was evaluated on 2026-07-21 but its `swetest`
target failed to compile because `spmoon` was undeclared in `swetest.c`.
Astraeus therefore pins the older, buildable release instead of carrying an
unreviewed patch to the external authority.

## Reproduction

Download the archive for the exact commit into a temporary directory, verify
its SHA-256, extract it, and run `make swetest`. Do not copy the source,
executable, or ephemeris data into this repository.

Run the tropical reference from the extracted source directory:

```text
./swetest -b1.1.2000 -ut12:00:00 -p0123456789mt -emos -speed -fPTlbRs -g, -head -house0.0000,51.4779,P
```

Run the sidereal reference by adding `-sid1` after `-emos`:

```text
./swetest -b1.1.2000 -ut12:00:00 -p0123456789mt -emos -sid1 -speed -fPTlbRs -g, -head -house0.0000,51.4779,P
```

Run the New York tropical reference with Koch houses:

```text
./swetest -b8.4.2024 -ut18:00:00 -p0123456789mt -emos -speed -fPTlbRs -g, -head -house-74.0060,40.7128,K
```

Run its Fagan–Bradley sidereal pair by adding `-sid0` after `-emos`.

Redirect stdout to the corresponding `.stdout` file. The JSON source record
contains its expected SHA-256. Tests verify the transcript hash, parse the
selected rows, and compare every requested position, cusp, Ascendant, and MC
with the normalized JSON values.

The format uses schema version 1 and rejects unknown top-level and expected
fields. Angular and speed tolerance is `1e-6` degrees (or degrees/day), and
distance tolerance is `1e-9` AU. Longitude and house comparisons account for
wrap-around at 0°/360°.

## Swiss-file suite

The two `j2000-greenwich-swiss-*.stdout` fixtures exercise Sun, Moon, Chiron,
houses, speed, and tropical/Lahiri sidereal configuration with `-eswe`.
`SWISS_PROVENANCE.md` records the immutable data revision and hashes. Run the
selected adapter suite without copying data into the repository:

```text
ASTRAEUS_SWISS_EPHEMERIS_PATH=/path/to/pinned-ephe \
  cargo test -p astraeus-swiss --test adapter -- --ignored --exact \
  swiss_files_match_tropical_and_sidereal_references
```

The manual `Swiss file verification` GitHub workflow runs the same command on
a self-hosted runner labeled `astraeus-swiss`. The protected
`swiss-ephemeris-verification` environment must define the
`ASTRAEUS_SWISS_EPHEMERIS_PATH` variable, pointing to runner-local pinned data.
Ordinary hosted CI never receives or caches the licensed ephemeris files.
Use the [ephemeral runner procedure](EPHEMERAL_RUNNER.md) for each selected
verification run; no self-hosted runner should remain registered afterward.
