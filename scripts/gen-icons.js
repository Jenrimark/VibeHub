// 生成 VibeHub 图标：纯色圆角 + 中心圆点的简单图标。
// 无第三方依赖，手写 PNG (zlib via node:zlib) 与 ICO。
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");

const outDir = path.join(__dirname, "..", "src-tauri", "icons");
fs.mkdirSync(outDir, { recursive: true });

// 在 size x size 画布上绘制：深色圆角背景 + 蓝色中心点。返回 RGBA Buffer。
function draw(size) {
  const buf = Buffer.alloc(size * size * 4);
  const r = size * 0.22; // 圆角半径
  const cx = size / 2,
    cy = size / 2;
  const dotR = size * 0.16;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = (y * size + x) * 4;
      // 圆角矩形内部判断
      const inside = roundedRectInside(x + 0.5, y + 0.5, size, r);
      if (!inside) {
        buf[i + 3] = 0; // 透明
        continue;
      }
      // 中心圆点
      const dd = Math.hypot(x + 0.5 - cx, y + 0.5 - cy);
      if (dd <= dotR) {
        buf[i] = 0x3b;
        buf[i + 1] = 0x82;
        buf[i + 2] = 0xf6; // 蓝
        buf[i + 3] = 255;
      } else {
        buf[i] = 0x16;
        buf[i + 1] = 0x16;
        buf[i + 2] = 0x1a; // 深底
        buf[i + 3] = 255;
      }
    }
  }
  return buf;
}

function roundedRectInside(x, y, size, r) {
  const min = r,
    max = size - r;
  if (x >= min && x <= max) return y >= 0 && y <= size;
  if (y >= min && y <= max) return x >= 0 && x <= size;
  // 四角
  const cxr = x < min ? min : max;
  const cyr = y < min ? min : max;
  return Math.hypot(x - cxr, y - cyr) <= r;
}

// 把 RGBA buffer 编码为 PNG。
function encodePNG(rgba, size) {
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  function chunk(type, data) {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length, 0);
    const typeBuf = Buffer.from(type, "ascii");
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])) >>> 0, 0);
    return Buffer.concat([len, typeBuf, data, crc]);
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  // 加 filter byte (0) 每行
  const stride = size * 4;
  const raw = Buffer.alloc((stride + 1) * size);
  for (let y = 0; y < size; y++) {
    raw[y * (stride + 1)] = 0;
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, y * stride + stride);
  }
  const idat = zlib.deflateSync(raw);

  return Buffer.concat([
    sig,
    chunk("IHDR", ihdr),
    chunk("IDAT", idat),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// CRC32
const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

// ICO：包含一个 PNG 帧（Vista+ 支持 PNG 压缩 ICO）。
function encodeICO(pngBuf, size) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type icon
  header.writeUInt16LE(1, 4); // count
  const entry = Buffer.alloc(16);
  entry[0] = size >= 256 ? 0 : size; // width
  entry[1] = size >= 256 ? 0 : size; // height
  entry[2] = 0; // palette
  entry[3] = 0;
  entry.writeUInt16LE(1, 4); // planes
  entry.writeUInt16LE(32, 6); // bpp
  entry.writeUInt32LE(pngBuf.length, 8);
  entry.writeUInt32LE(6 + 16, 12); // offset
  return Buffer.concat([header, entry, pngBuf]);
}

const sizes = [32, 128, 256];
const pngs = {};
for (const s of sizes) {
  const png = encodePNG(draw(s), s);
  pngs[s] = png;
}

fs.writeFileSync(path.join(outDir, "32x32.png"), pngs[32]);
fs.writeFileSync(path.join(outDir, "128x128.png"), pngs[128]);
fs.writeFileSync(path.join(outDir, "128x128@2x.png"), pngs[256]);
fs.writeFileSync(path.join(outDir, "icon.png"), pngs[256]);
fs.writeFileSync(path.join(outDir, "icon.ico"), encodeICO(pngs[256], 256));

console.log("图标已生成到", outDir);
