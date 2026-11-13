// Minimal SVG line chart for simulated dashboards (no chart library needed).

import { h, s } from './dom.js';

const PALETTE = ['#7c83ff', '#22c3a6', '#f5a524', '#ef5da8', '#4cc9f0', '#a3e635'];

export function lineChart({ labels = [], series = [], unit = '', thresholds = [] }, { height = 180 } = {}) {
  const width = 640;
  const pad = { l: 44, r: 12, t: 10, b: 24 };
  const all = series.flatMap((x) => x.points).concat(thresholds);
  const min = Math.min(0, ...all);
  const max = niceCeil(Math.max(1e-9, ...all) * 1.05);
  const n = Math.max(...series.map((x) => x.points.length), labels.length, 2);
  const x = (i) => pad.l + (i * (width - pad.l - pad.r)) / (n - 1);
  const y = (v) => pad.t + (1 - (v - min) / (max - min)) * (height - pad.t - pad.b);
  const ticks = 4;
  const grid = [];
  for (let k = 0; k <= ticks; k++) {
    const v = min + ((max - min) * k) / ticks;
    grid.push(
      s('line', { x1: pad.l, x2: width - pad.r, y1: y(v), y2: y(v), class: 'grid' }),
      s('text', { x: pad.l - 6, y: y(v) + 4, 'text-anchor': 'end', class: 'axis' }, fmt(v))
    );
  }
  const step = Math.max(1, Math.ceil(labels.length / 6));
  const xLabels = labels.map((l, i) => (i % step === 0 ? s('text', { x: x(i), y: height - 6, 'text-anchor': 'middle', class: 'axis' }, l) : null));
  const lines = series.map((ser, si) =>
    s('polyline', {
      points: ser.points.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(' '),
      fill: 'none',
      stroke: PALETTE[si % PALETTE.length],
      'stroke-width': 2,
      'stroke-linejoin': 'round',
    })
  );
  const th = thresholds.map((t) => s('line', { x1: pad.l, x2: width - pad.r, y1: y(t), y2: y(t), class: 'threshold' }));
  const svg = s('svg', { viewBox: `0 0 ${width} ${height}`, class: 'chart', role: 'img', 'aria-label': series.map((x) => x.name).join(', ') }, grid, th, lines, xLabels);
  const legend = h(
    'div',
    { class: 'legend' },
    series.map((ser, si) => {
      const lastV = ser.points[ser.points.length - 1];
      return h('span', null, h('i', { style: { background: PALETTE[si % PALETTE.length] } }), `${ser.name}`, h('b', null, ` ${fmt(lastV)}${unit ? ' ' + unit : ''}`));
    })
  );
  return h('div', { class: 'chart-wrap' }, svg, legend);
}

function fmt(v) {
  if (v === undefined || v === null || Number.isNaN(v)) return '';
  const a = Math.abs(v);
  if (a >= 1000) return `${(v / 1000).toFixed(1)}k`;
  if (a >= 100) return v.toFixed(0);
  if (a >= 10) return v.toFixed(1);
  return v.toFixed(2);
}

// Round an axis maximum up to 1, 2, 2.5 or 5 × 10^n so ticks read cleanly.
function niceCeil(v) {
  const p = Math.pow(10, Math.floor(Math.log10(v)));
  for (const m of [1, 2, 2.5, 5, 10]) if (m * p >= v) return m * p;
  return 10 * p;
}
