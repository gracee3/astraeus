# Exact event solving

`astraeus-events` finds previous, nearest, or next angular events with a
position-only scan-and-bracket bisection solver. Houses and the full chart are
calculated once, after the exact instant is known. Exact ties for nearest
select the earlier event. The default acceptance limits are one second in time
and `1e-5` degree angular residual.

Supported generic events are planetary returns, new/full moons, planetary sign
ingresses, and the four tropical seasonal points. Return targets are derived
inside the engine from the embedded natal chart. Birth-epoch-ecliptic returns
use the Swiss user-defined sidereal reference plane at the natal TT epoch;
callers do not supply or precession-correct a target longitude.

Each result includes solver metadata and an ordinary derived chart cast at the
exact instant and caller-provided location. Event artifacts embed their exact
position sample and, for returns, their natal target sample; all residual and
target fields are revalidated during deserialization. Global eclipse maxima
use a separate Swiss-native boundary.
