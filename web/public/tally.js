// Tally, soonbuilt's buddy, drawn in plain DOM. Same geometry as TallyReel's tally.tsx
// (face parts on a 96px note, scaled), so Tally looks identical everywhere.

export const PAPER = { peach: '#ffe2cc', butter: '#fff0b0', mint: '#d7f1e0', sky: '#dde8ff', lilac: '#ebe1ff' };
export const MOOD_PAPER = { hello: 'peach', listening: 'butter', pointing: 'sky', working: 'sky', ready: 'mint', needs: 'peach' };
const INK = '#1c1917';
const CHEEK = 'rgba(240,106,29,.28)';

function parts(mood, k) {
  const px = (v) => `${v * k}px`;
  const eyes = (dx = 0, dy = 0) => [
    [25.9 + dx, 34.6 + dy, 10.6, 15.4, `border-radius:${px(5.8)};background:${INK}`],
    [54.7 + dx, 34.6 + dy, 10.6, 15.4, `border-radius:${px(5.8)};background:${INK}`],
  ];
  const cheeks = [
    [12.5, 53.8, 11.5, 6.7, `border-radius:50%;background:${CHEEK}`],
    [71, 53.8, 11.5, 6.7, `border-radius:50%;background:${CHEEK}`],
  ];
  const smile = [39.4, 55.7, 17.3, 8.6, `border-bottom:${px(4.3)} solid ${INK};border-radius:0 0 ${px(11.5)} ${px(11.5)};box-sizing:border-box`];
  const flat = [40.3, 63.4, 15.4, 3.8, `border-radius:${px(1.9)};background:${INK}`];
  switch (mood) {
    case 'listening':
      return [...eyes(5.8, -7.7), ...cheeks, smile];
    case 'working':
      return [
        [25, 41.3, 14.4, 4.3, `border-radius:${px(2.9)};background:${INK}`],
        [53.8, 41.3, 14.4, 4.3, `border-radius:${px(2.9)};background:${INK}`],
        ...cheeks,
        flat,
      ];
    case 'ready': {
      const lid = `border-top:${px(4.8)} solid ${INK};border-radius:${px(11.5)} ${px(11.5)} 0 0;box-sizing:border-box`;
      return [[25, 38.4, 14.4, 7.7, lid], [53.8, 38.4, 14.4, 7.7, lid], ...cheeks, [36.5, 55.7, 23, 12.5, `border-radius:0 0 ${px(13.4)} ${px(13.4)};background:${INK}`]];
    }
    case 'needs':
      return [...eyes(), ...cheeks, flat];
    default:
      return [...eyes(), ...cheeks, smile];
  }
}

/** Returns Tally as an HTML string. */
export function tally({ size = 48, mood = 'hello', paper, tilt = -4, tape, shadow = true } = {}) {
  const k = size / 96;
  const px = (v) => `${v * k}px`;
  const bg = PAPER[paper || MOOD_PAPER[mood]];
  const withTape = tape ?? size >= 40;
  const sh = shadow
    ? `0 1px 1px rgba(28,25,23,.06),0 ${px(21.1)} ${px(34.6)} -${px(19.2)} rgba(28,25,23,.4)`
    : '0 0 0 1px rgba(28,25,23,.1)';
  const tp = withTape
    ? `<span class="tp" style="left:${px(26.9)};top:${px(-7.7)};width:${px(42.2)};height:${px(14.4)};background:rgba(255,255,255,.72);box-shadow:0 1px 2px rgba(0,0,0,.08);transform:rotate(-4deg)"></span>`
    : '';
  const face = parts(mood, k)
    .map(([l, t, w, h, css]) => `<span class="tp" style="left:${px(l)};top:${px(t)};width:${px(w)};height:${px(h)};${css}"></span>`)
    .join('');
  return `<span class="tally" data-mood="${mood}" aria-hidden="true" style="width:${size}px;height:${size}px;background:${bg};border-radius:${px(6.7)} ${px(6.7)} ${px(6.7)} ${px(25)};box-shadow:${sh};transform:rotate(${tilt}deg)">${tp}${face}</span>`;
}

/** Renders every `<span data-tally="size,mood,paper,tilt,tape">` on the page. */
export function renderAll(root = document) {
  root.querySelectorAll('[data-tally]').forEach((el) => {
    const [size, mood, paper, tilt, tape] = el.dataset.tally.split(',');
    el.innerHTML = tally({
      size: +size || 48,
      mood: mood || 'hello',
      paper: paper || undefined,
      tilt: tilt ? +tilt : -4,
      tape: tape === '1' ? true : tape === '0' ? false : undefined,
    });
  });
}
