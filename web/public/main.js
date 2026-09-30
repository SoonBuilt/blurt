import { renderAll, tally } from './tally.js';

/* ---------- downloads ----------
 * Fill these in when a release is published (public downloads repo), and the buttons
 * switch from "coming soon" to real downloads. Nothing else needs to change. */
const REPO = 'https://github.com/danishs360/blurt';
const RELEASE = {
  version: '0.1.1',
  mac: `${REPO}/releases/download/v0.1.1/Blurt_0.1.1_aarch64.dmg`,
  windows: `${REPO}/releases/download/v0.1.1/Blurt_0.1.1_x64-setup.exe`,
};

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;

const isWin = /Windows/.test(navigator.userAgent);
const isTouch = matchMedia('(hover: none)').matches;
const KEYS = isWin
  ? { talk: 'Ctrl', talkSide: 'RIGHT CTRL', ai: 'Shift', talkCode: 'ControlRight', talkKey: 'Control' }
  : { talk: '⌥', talkSide: 'RIGHT OPTION', ai: '⇧', talkCode: 'AltRight', talkKey: 'Alt' };

renderAll();
$$('[data-k]').forEach((el) => (el.textContent = KEYS[el.dataset.k]));
if (isTouch) $$('[data-hint-kb]').forEach((el) => el.remove());

/* ---------- download buttons ---------- */
const APPLE = '<svg class="os-ico" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M16.37 12.64c-.02-2.1 1.72-3.12 1.8-3.17-.98-1.44-2.51-1.64-3.05-1.66-1.3-.13-2.54.77-3.2.77-.66 0-1.68-.75-2.76-.73-1.42.02-2.73.83-3.46 2.1-1.48 2.56-.38 6.35 1.06 8.43.7 1.02 1.54 2.16 2.63 2.12 1.06-.04 1.46-.68 2.73-.68 1.28 0 1.64.68 2.76.66 1.14-.02 1.86-1.04 2.55-2.06.8-1.18 1.13-2.32 1.15-2.38-.03-.01-2.2-.85-2.21-3.35ZM14.27 6.47c.58-.71.98-1.69.87-2.67-.84.03-1.86.56-2.46 1.26-.54.62-1.01 1.62-.89 2.58.94.07 1.9-.48 2.48-1.17Z"/></svg>';
const WIN = '<svg class="os-ico" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M3 5.1 10.4 4v7.2H3V5.1Zm0 13.8 7.4 1.1v-7.1H3v6Zm8.3 1.2L21 21.5V12.9h-9.7v7.2Zm0-16.2v7.3H21V2.5l-9.7 1.4Z"/></svg>';

function dlButton(os, primary) {
  const url = os === 'mac' ? RELEASE.mac : RELEASE.windows;
  const name = os === 'mac' ? 'Mac' : 'Windows';
  const note = os === 'mac' ? 'Apple Silicon' : '64-bit';
  const cls = `btn ${primary ? 'btn-pri' : 'btn-sec'}`;
  const icon = os === 'mac' ? APPLE : WIN;
  return url
    ? `<a class="${cls}" href="${url}">${icon}Download for ${name} <small>${note}</small></a>`
    : `<span class="${cls} soon" aria-disabled="true">${icon}${name} <small>coming soon</small></span>`;
}
const order = isWin ? ['windows', 'mac'] : ['mac', 'windows'];
$$('[data-downloads]').forEach((el) => (el.innerHTML = order.map((os, i) => dlButton(os, i === 0)).join('')));
if (!RELEASE.mac && !RELEASE.windows) {
  const note = $('[data-dl-note]');
  if (note) note.textContent = "We're testing the first build right now. Your voice will be turned into text on your own computer, with no account needed.";
  $$('[data-dl-nav]').forEach((a) => (a.textContent = 'Coming soon'));
}

/* ---------- nav + reveal ---------- */
const nav = $('#nav');
addEventListener('scroll', () => nav.classList.toggle('scrolled', scrollY > 8), { passive: true });
const io = new IntersectionObserver(
  (entries) => entries.forEach((e) => e.isIntersecting && (e.target.classList.add('in'), io.unobserve(e.target))),
  { threshold: 0.12 },
);
$$('.reveal').forEach((el) => io.observe(el));
// Never leave content hidden if the observer is slow to fire (e.g. background tabs).
setTimeout(() => $$('.reveal').forEach((el) => el.classList.add('in')), 4000);

/* ---------- live demo ---------- */
const SCENES = {
  dictate: {
    incoming: 'Quick one: where are we on the landing page?',
    raw: "um so almost there uh the copy's done and the hero video lands this afternoon so we're on track for thursday",
    out: "Almost there! The copy's done and the hero video lands this afternoon, so we're on track for Thursday.",
  },
  ai: {
    incoming: 'Are we still good for Friday?',
    draft: "hey so we cant hit friday cause the api from your side still isnt working, told you this like 2 weeks ago",
    ask: 'make this polite but firm',
    out: "Unfortunately we won't make Friday: we're still waiting on the API from your side, which we flagged two weeks ago. Happy to agree a new date together.",
  },
  aiNoContext: {
    ask: 'write a quick reply saying we launch thursday',
    out: "We're on track! Everything's coming together and we launch on Thursday. 🚀",
  },
};

const stage = {
  composer: $('#composer'),
  incoming: $('#incoming'),
  pill: $('#pill'),
  heard: $('#heard'),
  talk: $('#talkKey'),
  shift: $('#shiftKey'),
};
let armed = false; // ⇧ toggled on
let holding = false;
let mode = 'dictate';
let hasSelection = false;
let holdStart = 0;
let waveTimer = 0;
let busy = false;
let prepPending = false; // ⇧ was toggled while a demo run was finishing

function setIncoming(t) {
  stage.incoming.textContent = t;
}
function placeholder() {
  stage.composer.classList.remove('focus');
  stage.composer.innerHTML = '<span class="ph">Message #launch-week</span>';
  hasSelection = false;
}
function showDraft() {
  stage.composer.classList.add('focus');
  stage.composer.innerHTML = `<mark>${SCENES.ai.draft}</mark>`;
  hasSelection = true;
}
function setArmed(on) {
  armed = on;
  stage.shift.classList.toggle('armed', on);
  stage.shift.setAttribute('aria-pressed', String(on));
  if (holding || busy) {
    prepPending = true;
    return;
  }
  prepPending = false;
  {
    if (on) {
      setIncoming(SCENES.ai.incoming);
      showDraft();
    } else {
      setIncoming(SCENES.dictate.incoming);
      placeholder();
    }
  }
}

function pill(html, cls = '') {
  stage.pill.className = `pill on ${cls}`;
  stage.pill.innerHTML = html;
}
function pillOff() {
  stage.pill.className = 'pill';
}
const T = (mood, paper, anim) => `<span class="slot ${anim || ''}">${tally({ size: 30, mood, paper, tilt: -5, tape: false, shadow: false })}</span>`;

function listeningPill() {
  const ai = mode === 'ai';
  const chip = ai && hasSelection ? `<span class="chip ctx">📎 ${SCENES.ai.draft.split(' ').length} words</span>` : '';
  pill(
    `${T('listening', ai ? 'lilac' : 'butter', 'bob')}<span class="wave">${'<i></i>'.repeat(16)}</span><span class="lbl">${ai ? 'Ask AI' : 'Listening'}</span>${chip}${ai ? '' : `<span class="sub">+${KEYS.ai} for AI</span>`}`,
    ai ? 'ai' : '',
  );
  clearInterval(waveTimer);
  let prev = Array(16).fill(0.2);
  waveTimer = setInterval(() => {
    // Speech-like movement: smoothed random levels with occasional pauses.
    const talking = Math.random() > 0.12;
    prev = prev.map((p) => p * 0.45 + (talking ? Math.random() : 0.05) * 0.55);
    $$('.wave i', stage.pill).forEach((b, i) => (b.style.height = `${4 + Math.round(prev[i] * 18)}px`));
  }, 90);
}

function pressStart(fromUser) {
  if (holding || busy) return;
  if (fromUser) stopAutoplay();
  holding = true;
  holdStart = performance.now();
  mode = armed ? 'ai' : 'dictate';
  stage.talk.classList.add('down');
  if (mode === 'dictate' && !stage.composer.querySelector('.txt')) {
    stage.composer.classList.add('focus');
    stage.composer.innerHTML = '<span class="caret"></span>';
  }
  listeningPill();
}

function switchToAi() {
  if (!holding || mode === 'ai') return;
  mode = 'ai';
  listeningPill();
}

async function pressEnd() {
  if (!holding) return;
  holding = false;
  stage.talk.classList.remove('down');
  clearInterval(waveTimer);
  if (performance.now() - holdStart < 450) {
    pill(`${T('needs', 'peach', 'pop')}<span class="lbl">Hold it down while you talk</span>`);
    await sleep(1400);
    pillOff();
    if (!armed) placeholder();
    return;
  }
  busy = true;
  const ai = mode === 'ai';
  pill(`${T('working', ai ? 'lilac' : 'sky', 'wig')}<span class="lbl">${ai ? 'Listening back…' : 'Writing it down…'}</span><span class="sub">on-device</span>`, ai ? 'ai' : '');
  await sleep(ai ? 450 : 520);

  if (!ai) {
    const s = SCENES.dictate;
    pillOff();
    await typeOut(s.out);
    stage.heard.innerHTML = `<b>Heard</b> “${s.raw.replace(/\b(um|uh)\b/g, '<s>$1</s>')}” <b>→ typed it clean</b>`;
    pill(`${T('ready', 'mint', 'pop')}<span class="lbl">✓ ${s.out.split(' ').length} words</span>`);
  } else {
    const s = hasSelection ? SCENES.ai : SCENES.aiNoContext;
    const ctx = hasSelection ? `<span class="chip ctx">📎 ${SCENES.ai.draft.split(' ').length} words</span>` : '';
    pill(`${T('working', 'lilac', 'wig')}<span class="lbl">On it…</span><span class="chip">“${s.ask}”</span>${ctx}`, 'ai');
    await sleep(1150);
    pillOff();
    if (hasSelection) {
      stage.composer.innerHTML = `<span class="txt new">${s.out}</span>`;
      hasSelection = false;
    } else {
      await typeOut(s.out);
    }
    stage.heard.innerHTML = `<b>Heard</b> “${s.ask}” <b>→ ${usingCtx(s) ? 'rewrote your highlight' : 'wrote it for you'}</b>`;
    pill(`${T('ready', 'mint', 'pop')}<span class="lbl">✓ Done</span>`);
  }
  await sleep(1300);
  pillOff();
  busy = false;
  if (prepPending) setArmed(armed);
}
const usingCtx = (s) => s === SCENES.ai;

async function typeOut(text) {
  stage.composer.classList.add('focus');
  const span = document.createElement('span');
  span.className = 'txt';
  const caret = document.createElement('span');
  caret.className = 'caret';
  stage.composer.innerHTML = '';
  stage.composer.append(span, caret);
  if (reduced) {
    span.textContent = text;
    return;
  }
  for (const w of text.split(/(\s+)/)) {
    span.textContent += w;
    await sleep(34);
  }
}

/* pointer: press-and-hold the on-screen key */
stage.talk.addEventListener('pointerdown', (e) => {
  e.preventDefault();
  stage.talk.setPointerCapture(e.pointerId);
  pressStart(true);
});
['pointerup', 'pointercancel'].forEach((ev) => stage.talk.addEventListener(ev, () => pressEnd()));
stage.talk.addEventListener('keydown', (e) => {
  if ((e.key === ' ' || e.key === 'Enter') && !e.repeat) {
    e.preventDefault();
    pressStart(true);
  }
});
stage.talk.addEventListener('keyup', (e) => {
  if (e.key === ' ' || e.key === 'Enter') pressEnd();
});
stage.shift.addEventListener('click', () => {
  stopAutoplay();
  if (holding) switchToAi();
  else setArmed(!armed);
});

/* real keyboard: hold Right ⌥ (Mac) or Right Ctrl (Windows); Shift switches to AI */
let kbTimer = 0;
addEventListener('keydown', (e) => {
  if (e.repeat) return;
  if (e.key === KEYS.talkKey && (e.code === KEYS.talkCode || e.location === 2)) {
    e.preventDefault();
    const withShift = e.shiftKey;
    kbTimer = setTimeout(() => {
      if (withShift && !armed) setArmed(true);
      pressStart(true);
    }, 140);
  } else if (e.key === 'Shift' && holding) {
    switchToAi();
  } else if (e.key !== 'Shift') {
    clearTimeout(kbTimer); // it was a shortcut, not a hold
  }
});
addEventListener('keyup', (e) => {
  if (e.key === KEYS.talkKey) {
    clearTimeout(kbTimer);
    pressEnd();
  }
});
addEventListener('blur', () => {
  clearTimeout(kbTimer);
  pressEnd();
});

/* autoplay until the visitor takes over */
let autoplay = !reduced;
let autoToken = 0;
function stopAutoplay() {
  if (!autoplay) return;
  autoplay = false;
  autoToken++;
}
async function runAutoplay() {
  const tok = ++autoToken;
  const alive = () => autoplay && tok === autoToken;
  await sleep(1500);
  while (alive()) {
    setArmed(false);
    await sleep(700);
    if (!alive()) return;
    pressStart(false);
    await sleep(2300);
    if (!alive()) return pressEnd();
    await pressEnd();
    await sleep(2600);
    if (!alive()) return;
    setArmed(true);
    await sleep(1300);
    if (!alive()) return;
    pressStart(false);
    await sleep(1700);
    if (!alive()) return pressEnd();
    await pressEnd();
    await sleep(3200);
    stage.heard.textContent = '';
  }
}
setIncoming(SCENES.dictate.incoming);
if (autoplay) {
  // Only animate while the demo is on screen.
  new IntersectionObserver(([e]) => {
    if (e.isIntersecting && autoplay && autoToken === 0) runAutoplay();
  }).observe($('#stage'));
}
