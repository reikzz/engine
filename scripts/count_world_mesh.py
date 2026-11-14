import json
import math

def project_mercator(lon, lat):
    lat = max(-80.0, min(84.0, lat))
    lat_rad = math.radians(lat)
    x = lon * 15.0
    y = -math.log(math.tan(math.pi / 4.0 + lat_rad * 0.5)) * 859.4367
    return (round(x, 1), round(y, 1))

with open('data/map/countries.geojson', 'r', encoding='utf-8') as f:
    countries = json.load(f)

with open('data/map/world_subdivisions.geojson', 'r', encoding='utf-8') as f:
    subdivisions = json.load(f)

def extract_outer_rings(geom):
    gtype = geom['type']
    coords = geom['coordinates']
    rings = []
    if gtype == 'Polygon':
        if len(coords) > 0 and len(coords[0]) >= 4:
            rings.append(coords[0])
    elif gtype == 'MultiPolygon':
        for poly in coords:
            if len(poly) > 0 and len(poly[0]) >= 4:
                rings.append(poly[0])
    return rings

total_rings = 0
total_pts = 0

# USA, RUS, CHN subdivisions
for feat in subdivisions['features']:
    adm0 = feat['properties'].get('adm0_a3')
    if adm0 in ('USA', 'RUS', 'CHN'):
        rings = extract_outer_rings(feat['geometry'])
        for r in rings:
            total_rings += 1
            total_pts += len(r)

# Rest of countries
for feat in countries['features']:
    iso = feat['properties'].get('ISO_A3') or feat['properties'].get('ADM0_A3')
    if iso in ('USA', 'RUS', 'CHN', 'ATA'): # ATA is Antarctica
        continue
    rings = extract_outer_rings(feat['geometry'])
    for r in rings:
        total_rings += 1
        total_pts += len(r)

print(f"Total polygon rings across entire world map: {total_rings}")
print(f"Total vertices across entire world map: {total_pts}")
