# Exact event solving

`astraeus-events` finds previous, nearest, or next angular events with a
scan-and-bracket bisection solver. Exact ties for nearest select the earlier
event. The default acceptance limits are one second in time and `1e-5` degree
angular residual.

Supported generic events are planetary returns, new/full moons, planetary sign
ingresses, and the four tropical seasonal points. Return artifacts explicitly
record whether the supplied target is in the configured zodiac or is a
birth-epoch-ecliptic target that the caller has precession-corrected.

Each result includes solver metadata and an ordinary derived chart cast at the
exact instant and caller-provided location. Global eclipse maxima require the
Swiss Ephemeris native eclipse boundary and are tracked separately from this
generic angular solver.
