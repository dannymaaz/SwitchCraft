import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outputDir = path.join(root, 'src-tauri', 'installer');
fs.mkdirSync(outputDir, { recursive: true });

function createCanvas(width, height) {
  const pixels = new Uint8Array(width * height * 3);

  function setPixel(x, y, [r, g, b]) {
    x = Math.round(x);
    y = Math.round(y);
    if (x < 0 || y < 0 || x >= width || y >= height) return;
    const index = (y * width + x) * 3;
    pixels[index] = r;
    pixels[index + 1] = g;
    pixels[index + 2] = b;
  }

  function blendPixel(x, y, color, alpha = 1) {
    x = Math.round(x);
    y = Math.round(y);
    if (x < 0 || y < 0 || x >= width || y >= height) return;
    const index = (y * width + x) * 3;
    pixels[index] = Math.round(pixels[index] * (1 - alpha) + color[0] * alpha);
    pixels[index + 1] = Math.round(pixels[index + 1] * (1 - alpha) + color[1] * alpha);
    pixels[index + 2] = Math.round(pixels[index + 2] * (1 - alpha) + color[2] * alpha);
  }

  function fill(colorFn) {
    for (let y = 0; y < height; y += 1) {
      for (let x = 0; x < width; x += 1) setPixel(x, y, colorFn(x, y));
    }
  }

  function circle(cx, cy, radius, color, alpha = 1) {
    const minX = Math.floor(cx - radius);
    const maxX = Math.ceil(cx + radius);
    const minY = Math.floor(cy - radius);
    const maxY = Math.ceil(cy + radius);
    const radiusSq = radius * radius;
    for (let y = minY; y <= maxY; y += 1) {
      for (let x = minX; x <= maxX; x += 1) {
        const dx = x - cx;
        const dy = y - cy;
        if (dx * dx + dy * dy <= radiusSq) blendPixel(x, y, color, alpha);
      }
    }
  }

  function line(x0, y0, x1, y1, thickness, color, alpha = 1) {
    const dx = x1 - x0;
    const dy = y1 - y0;
    const steps = Math.max(Math.abs(dx), Math.abs(dy), 1) * 2;
    for (let i = 0; i <= steps; i += 1) {
      const t = i / steps;
      circle(x0 + dx * t, y0 + dy * t, thickness / 2, color, alpha);
    }
  }

  function glow(cx, cy, radius, color, strength = 0.25) {
    const minX = Math.floor(cx - radius);
    const maxX = Math.ceil(cx + radius);
    const minY = Math.floor(cy - radius);
    const maxY = Math.ceil(cy + radius);
    for (let y = minY; y <= maxY; y += 1) {
      for (let x = minX; x <= maxX; x += 1) {
        const distance = Math.hypot(x - cx, y - cy);
        if (distance > radius) continue;
        const alpha = (1 - distance / radius) ** 2 * strength;
        blendPixel(x, y, color, alpha);
      }
    }
  }

  return { width, height, pixels, fill, line, circle, glow, blendPixel };
}

function writeBmp(filename, canvas) {
  const { width, height, pixels } = canvas;
  const rowStride = Math.ceil((width * 3) / 4) * 4;
  const pixelBytes = rowStride * height;
  const buffer = Buffer.alloc(54 + pixelBytes);

  buffer.write('BM', 0, 2, 'ascii');
  buffer.writeUInt32LE(buffer.length, 2);
  buffer.writeUInt32LE(54, 10);
  buffer.writeUInt32LE(40, 14);
  buffer.writeInt32LE(width, 18);
  buffer.writeInt32LE(height, 22);
  buffer.writeUInt16LE(1, 26);
  buffer.writeUInt16LE(24, 28);
  buffer.writeUInt32LE(0, 30);
  buffer.writeUInt32LE(pixelBytes, 34);
  buffer.writeInt32LE(2835, 38);
  buffer.writeInt32LE(2835, 42);

  for (let sourceY = 0; sourceY < height; sourceY += 1) {
    const destY = height - 1 - sourceY;
    const rowStart = 54 + destY * rowStride;
    for (let x = 0; x < width; x += 1) {
      const source = (sourceY * width + x) * 3;
      const dest = rowStart + x * 3;
      buffer[dest] = pixels[source + 2];
      buffer[dest + 1] = pixels[source + 1];
      buffer[dest + 2] = pixels[source];
    }
  }

  fs.writeFileSync(path.join(outputDir, filename), buffer);
}

const GOLD = [217, 173, 83];
const GOLD_LIGHT = [240, 211, 137];
const GOLD_DARK = [143, 100, 20];

function drawRoutingMark(canvas, cx, cy, scale = 1) {
  const t = 5 * scale;
  const left = cx - 38 * scale;
  const right = cx + 38 * scale;
  const top = cy - 26 * scale;
  const bottom = cy + 26 * scale;

  canvas.line(left, top, cx + 20 * scale, top, t, GOLD_LIGHT, 0.96);
  canvas.line(cx + 20 * scale, top, right, cy - 8 * scale, t, GOLD, 0.96);
  canvas.line(right, cy - 8 * scale, cx + 18 * scale, cy, t, GOLD, 0.96);
  canvas.line(cx + 18 * scale, cy, cx - 18 * scale, cy, t, GOLD, 0.96);
  canvas.line(cx - 18 * scale, cy, left, cy + 10 * scale, t, GOLD, 0.96);
  canvas.line(left, cy + 10 * scale, cx - 20 * scale, bottom, t, GOLD_DARK, 1);
  canvas.line(cx - 20 * scale, bottom, right, bottom, t, GOLD, 1);

  canvas.line(right - 10 * scale, top - 8 * scale, right, top, t * 0.72, GOLD_LIGHT, 1);
  canvas.line(right - 10 * scale, top + 8 * scale, right, top, t * 0.72, GOLD_LIGHT, 1);
  canvas.line(left + 10 * scale, bottom - 8 * scale, left, bottom, t * 0.72, GOLD, 1);
  canvas.line(left + 10 * scale, bottom + 8 * scale, left, bottom, t * 0.72, GOLD, 1);
  canvas.circle(cx, cy, 4.2 * scale, GOLD_LIGHT, 1);
}

function buildHeader() {
  const canvas = createCanvas(150, 57);
  canvas.fill((x, y) => {
    const shade = Math.round(8 + (1 - y / 57) * 7 + (x / 150) * 2);
    return [shade, shade, Math.max(7, shade - 2)];
  });
  canvas.glow(25, 28, 31, GOLD, 0.2);
  drawRoutingMark(canvas, 32, 28, 0.34);
  canvas.line(57, 18, 141, 18, 1.2, GOLD, 0.38);
  canvas.line(57, 28, 125, 28, 1.2, [123, 116, 103], 0.5);
  canvas.line(57, 37, 136, 37, 1.2, [75, 72, 67], 0.56);
  return canvas;
}

function buildSidebar() {
  const canvas = createCanvas(164, 314);
  canvas.fill((x, y) => {
    const vertical = 5 + Math.round((1 - y / 314) * 8);
    const edge = Math.round(Math.max(0, 4 - Math.abs(x - 82) / 28));
    return [vertical + edge, vertical + edge, Math.max(4, vertical + edge - 2)];
  });
  canvas.glow(82, 64, 88, GOLD, 0.16);
  canvas.glow(134, 250, 75, GOLD_DARK, 0.1);
  drawRoutingMark(canvas, 82, 126, 1.03);
  canvas.line(34, 210, 130, 210, 1, GOLD, 0.26);
  canvas.line(48, 226, 116, 226, 1, [112, 105, 91], 0.38);
  canvas.line(58, 241, 106, 241, 1, [74, 71, 65], 0.45);
  canvas.circle(82, 278, 2.4, GOLD, 0.75);
  return canvas;
}

writeBmp('header.bmp', buildHeader());
writeBmp('sidebar.bmp', buildSidebar());
console.log(`Generated NSIS branding in ${path.relative(root, outputDir)}`);
