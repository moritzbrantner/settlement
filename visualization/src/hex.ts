export type HexTile = {
  q: number;
  r: number;
};

type CubeTile = {
  x: number;
  y: number;
  z: number;
};

export const SIDE_LENGTH = 12;
export const MAP_RADIUS = SIDE_LENGTH - 1;
export const HEX_SIZE = 26;
const HEX_WIDTH = Math.sqrt(3) * HEX_SIZE;
const HEX_HEIGHT = HEX_SIZE * 2;

export function buildHexagon(radius: number) {
  const tiles: Array<HexTile & { key: string }> = [];
  for (let q = -radius; q <= radius; q += 1) {
    const rMin = Math.max(-radius, -q - radius);
    const rMax = Math.min(radius, -q + radius);
    for (let r = rMin; r <= rMax; r += 1) {
      tiles.push({ q, r, key: tileKey({ q, r }) });
    }
  }
  return tiles;
}

export function buildCenters(tiles: Array<HexTile & { key: string }>) {
  return tiles.map((tile) => {
    const pixel = axialToPixel(tile, HEX_SIZE);
    return {
      ...tile,
      ...pixel,
      corners: hexCorners(pixel.x, pixel.y, HEX_SIZE),
      distance: hexDistance(tile, { q: 0, r: 0 }),
    };
  });
}

export function axialToPixel(tile: HexTile, size: number) {
  return {
    x: size * Math.sqrt(3) * (tile.q + tile.r / 2),
    y: size * 1.5 * tile.r,
  };
}

export function hexDistance(from: HexTile, to: HexTile) {
  const a = axialToCube(from);
  const b = axialToCube(to);
  return Math.max(
    Math.abs(a.x - b.x),
    Math.abs(a.y - b.y),
    Math.abs(a.z - b.z),
  );
}

export function buildRoute(start: HexTile, goal: HexTile) {
  const distance = hexDistance(start, goal);
  if (distance === 0) {
    return [start];
  }

  const startCube = axialToCube(start);
  const goalCube = axialToCube(goal);
  const route: HexTile[] = [];

  for (let index = 0; index <= distance; index += 1) {
    const progress = index / distance;
    const point = cubeRound({
      x: lerp(startCube.x, goalCube.x, progress),
      y: lerp(startCube.y, goalCube.y, progress),
      z: lerp(startCube.z, goalCube.z, progress),
    });
    const tile = cubeToAxial(point);
    if (route.at(-1)?.q !== tile.q || route.at(-1)?.r !== tile.r) {
      route.push(tile);
    }
  }

  return route;
}

export function measureBounds(tileCenters: Array<{ x: number; y: number }>) {
  const halfWidth = HEX_WIDTH / 2;
  const halfHeight = HEX_HEIGHT / 2;
  const rawMinX = Math.min(...tileCenters.map((tile) => tile.x - halfWidth));
  const rawMaxX = Math.max(...tileCenters.map((tile) => tile.x + halfWidth));
  const rawMinY = Math.min(...tileCenters.map((tile) => tile.y - halfHeight));
  const rawMaxY = Math.max(...tileCenters.map((tile) => tile.y + halfHeight));
  const padding = HEX_SIZE * 1.8;

  return {
    minX: round(rawMinX - padding),
    minY: round(rawMinY - padding),
    width: round(rawMaxX - rawMinX + padding * 2),
    height: round(rawMaxY - rawMinY + padding * 2),
  };
}

export function hexCorners(centerX: number, centerY: number, size: number) {
  const points: string[] = [];
  for (let index = 0; index < 6; index += 1) {
    const angle = (60 * index - 30) * (Math.PI / 180);
    const x = centerX + size * Math.cos(angle);
    const y = centerY + size * Math.sin(angle);
    points.push(`${round(x)},${round(y)}`);
  }
  return points.join(" ");
}

export function toPercent(value: number, min: number, span: number) {
  return ((value - min) / span) * 100;
}

export function round(value: number) {
  return Number(value.toFixed(2));
}

export function tileKey(tile: HexTile) {
  return `${tile.q},${tile.r}`;
}

function axialToCube(tile: HexTile): CubeTile {
  return {
    x: tile.q,
    z: tile.r,
    y: -tile.q - tile.r,
  };
}

function cubeToAxial(tile: CubeTile): HexTile {
  return {
    q: tile.x,
    r: tile.z,
  };
}

function cubeRound(tile: CubeTile): CubeTile {
  let x = Math.round(tile.x);
  let y = Math.round(tile.y);
  let z = Math.round(tile.z);

  const xDiff = Math.abs(x - tile.x);
  const yDiff = Math.abs(y - tile.y);
  const zDiff = Math.abs(z - tile.z);

  if (xDiff > yDiff && xDiff > zDiff) {
    x = -y - z;
  } else if (yDiff > zDiff) {
    y = -x - z;
  } else {
    z = -x - y;
  }

  return { x, y, z };
}

function lerp(start: number, end: number, progress: number) {
  return start + (end - start) * progress;
}
