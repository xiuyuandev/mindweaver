#!/usr/bin/env node
/**
 * 生成一个合法的 .zmind 样例文件（纯 Node 实现，不依赖任何库）。
 *
 * 用途：a) 冒烟测试"最近文件/打开"路径；
 *      b) 跨实现互操作验证（Node 写 → Rust 读）；
 *      c) M5 性能/内存样本：`--nodes=5000` / `--nodes=20000`。
 *
 * 用法：node scripts/make-sample.mjs [输出路径] [--nodes=N]
 */
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

const args = process.argv.slice(2);
let outPath = 'target/smoke-sample.zmind';
let wanted = 0;
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--nodes' && i + 1 < args.length) {
    wanted = parseInt(args[++i], 10);
  } else if (a.startsWith('--nodes=')) {
    wanted = parseInt(a.slice('--nodes='.length), 10);
  } else {
    outPath = a;
  }
}
if (!Number.isFinite(wanted) || wanted < 0) wanted = 0;

// ---------- CRC32 ----------
const CRC_TABLE = (() => {
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
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

// ---------- 最小 ZIP 写入器（全部 STORED，无压缩） ----------
function zip(entries) {
  const chunks = [];
  const central = [];
  let offset = 0;
  const dosDate = ((2024 - 1980) << 9) | (1 << 5) | 1; // 2024-01-01

  for (const e of entries) {
    const name = Buffer.from(e.name, 'utf8');
    const data = Buffer.isBuffer(e.data) ? e.data : Buffer.from(e.data, 'utf8');
    const crc = crc32(data);
    const lh = Buffer.alloc(30);
    lh.writeUInt32LE(0x04034b50, 0);
    lh.writeUInt16LE(20, 4); // version needed
    lh.writeUInt16LE(0, 6); // flags
    lh.writeUInt16LE(0, 8); // method = stored
    lh.writeUInt16LE(0, 10); // time
    lh.writeUInt16LE(dosDate, 12);
    lh.writeUInt32LE(crc, 14);
    lh.writeUInt32LE(data.length, 18);
    lh.writeUInt32LE(data.length, 22);
    lh.writeUInt16LE(name.length, 26);
    lh.writeUInt16LE(0, 28);
    chunks.push(lh, name, data);
    central.push({ name, crc, size: data.length, offset });
    offset += lh.length + name.length + data.length;
  }

  const cdParts = [];
  let cdSize = 0;
  for (const c of central) {
    const cd = Buffer.alloc(46);
    cd.writeUInt32LE(0x02014b50, 0);
    cd.writeUInt16LE(20, 4); // version made by
    cd.writeUInt16LE(20, 6); // version needed
    cd.writeUInt16LE(0, 8); // flags
    cd.writeUInt16LE(0, 10); // method
    cd.writeUInt16LE(0, 12); // time
    cd.writeUInt16LE(dosDate, 14);
    cd.writeUInt32LE(c.crc, 16);
    cd.writeUInt32LE(c.size, 20);
    cd.writeUInt32LE(c.size, 24);
    cd.writeUInt16LE(c.name.length, 28);
    cd.writeUInt16LE(0, 30); // extra len
    cd.writeUInt16LE(0, 32); // comment len
    cd.writeUInt16LE(0, 34); // disk start
    cd.writeUInt16LE(0, 36); // internal attrs
    cd.writeUInt32LE(0, 38); // external attrs
    cd.writeUInt32LE(c.offset, 42);
    cdParts.push(cd, c.name);
    cdSize += cd.length + c.name.length;
  }

  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(0, 4);
  eocd.writeUInt16LE(0, 6);
  eocd.writeUInt16LE(central.length, 8);
  eocd.writeUInt16LE(central.length, 10);
  eocd.writeUInt32LE(cdSize, 12);
  eocd.writeUInt32LE(offset, 16);
  eocd.writeUInt16LE(0, 20);

  return Buffer.concat([...chunks, ...cdParts, eocd]);
}

// ---------- 样例文档 ----------
const now = Date.now();

/** 3 节点互操作样例（默认行为，保持不变）。 */
function smallContent() {
  return {
    schema: 1,
    rootId: 'sample-root',
    nodes: {
      'sample-root': {
        id: 'sample-root',
        parentId: null,
        children: ['sample-a', 'sample-b'],
        text: '跨实现样例',
        collapsed: false,
        modifiedAt: now,
      },
      'sample-a': {
        id: 'sample-a',
        parentId: 'sample-root',
        children: [],
        text: '由 Node 脚本生成',
        icon: 'priority-1',
        collapsed: false,
        modifiedAt: now,
      },
      'sample-b': {
        id: 'sample-b',
        parentId: 'sample-root',
        children: [],
        text: '由 Rust 读取',
        note: '这是跨实现互操作样例：Node 写入，应用打开。',
        collapsed: false,
        modifiedAt: now,
      },
    },
    theme: 'mint',
    layout: { kind: 'logicRight', siblingGap: 24, levelGap: 80, maxNodeWidth: 300 },
  };
}

/** 合成大树：宽度节奏与 `mw-render` 的 perf 例一致，文本有长短混排。 */
function bigContent(count) {
  const nodes = {};
  const root = {
    id: 'perf-root',
    parentId: null,
    children: [],
    text: `性能样本 · ${count} 节点`,
    collapsed: false,
    modifiedAt: now,
  };
  nodes[root.id] = root;
  const widths = [6, 8, 8, 5];
  const queue = [{ id: root.id, depth: 0 }];
  let next = 1;
  while (next < count && queue.length > 0) {
    const { id: parent, depth } = queue.shift();
    const width = widths[depth % widths.length];
    for (let i = 0; i < width && next < count; i++) {
      const id = `n${next}`;
      const long = next % 53 === 0;
      const node = {
        id,
        parentId: parent,
        children: [],
        text: long
          ? `分支主题 ${next} · 一段较长的中文标题用于排版与折行测量`
          : `分支 ${next}`,
        collapsed: false,
        modifiedAt: now,
      };
      if (next % 40 === 0) node.labels = ['重要'];
      if (next % 97 === 0) node.icon = 'priority-1';
      if (next % 211 === 0) node.note = '合成备注：用于测量备注角标与检查器路径。';
      nodes[parent].children.push(id);
      nodes[id] = node;
      queue.push({ id, depth: depth + 1 });
      next += 1;
    }
  }
  return {
    schema: 1,
    rootId: root.id,
    nodes,
    theme: 'mint',
    // 放射布局才把 5k 节点摊在二维里；logicRight 会叠成一根几千像素高的柱子。
    layout: { kind: 'mindmap', siblingGap: 24, levelGap: 80, maxNodeWidth: 300 },
  };
}

const content = wanted > 0 ? bigContent(wanted) : smallContent();
const nodeCount = Object.keys(content.nodes).length;
const title = wanted > 0 ? `性能样本 ${nodeCount}` : '跨实现样例';

const manifest = {
  formatVersion: 1,
  app: 'smoke-sample/1.0',
  docId: wanted > 0 ? `perf-${nodeCount}` : 'smoke-sample',
  title,
  createdAt: now,
  modifiedAt: now,
  nodeCount,
};

const bytes = zip([
  { name: 'mimetype', data: 'application/vnd.mindweaver.zmind' },
  { name: 'manifest.json', data: JSON.stringify(manifest, null, 2) },
  { name: 'content.json', data: JSON.stringify(content) },
]);

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, bytes);
console.log(`sample written: ${outPath} (${bytes.length} bytes)`);
