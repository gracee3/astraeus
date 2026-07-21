# Aspect calculations

`astraeus-core` detects aspects from validated ecliptic longitudes without
calling an ephemeris provider. The initial contract supports conjunction,
sextile, square, trine, and opposition with an explicit orb per aspect.

Detection uses the shortest separation on the 360-degree circle. Orb limits
are inclusive. Results use canonical object ordering and contain the measured
separation and absolute distance from exactitude. When configured windows
overlap, only the closest aspect is returned for a pair; configuration order
does not affect results, and canonical aspect-kind order breaks an exact tie.

The engine intentionally does not yet label aspects applying or separating.
That requires a separately tested convention for relative motion, stations,
and the 0/180-degree discontinuities.
