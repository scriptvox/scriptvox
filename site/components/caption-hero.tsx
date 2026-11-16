'use client';
import { useEffect, useState } from 'react';

const LINE = ['Write', 'the', 'script.', 'ScriptVox', 'makes', 'the', 'video.'];
const STEP = 0.34; // seconds between words
const START = 0.6;
const END = START + LINE.length * STEP + 1.2;

function timecode(t: number) {
  const f = Math.floor((t % 1) * 25);
  const s = Math.floor(t);
  const p = (n: number) => String(n).padStart(2, '0');
  return `00:00:${p(s)}:${p(f)}`;
}

export function CaptionHero() {
  const [take, setTake] = useState(0);
  const [t, setT] = useState(0);

  useEffect(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      setT(END);
      return;
    }
    const t0 = performance.now();
    let id = 0;
    const tick = (now: number) => {
      const s = Math.min((now - t0) / 1000, END);
      setT(s);
      if (s < END) id = requestAnimationFrame(tick);
    };
    id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(id);
  }, [take]);

  return (
    <div className="sv-frame relative aspect-[4/3] sm:aspect-[2.39/1] w-full p-5 sm:p-8 flex flex-col">
      <div className="flex justify-between font-mono text-xs text-[var(--sv-smoke)] tabular-nums">
        <span aria-hidden="true">{timecode(t)}</span>
        <button
          type="button"
          onClick={() => setTake((n) => n + 1)}
          className="hover:text-[var(--sv-chalk)] transition-colors"
        >
          Play again
        </button>
      </div>
      <h1
        key={take}
        className="mt-auto mx-auto max-w-[22ch] text-center font-extrabold leading-[1.02] tracking-[-0.02em] text-[clamp(2rem,6.4vw,5.4rem)] [font-stretch:112%]"
      >
        {LINE.map((w, i) => (
          <span key={i}>
            <span className="sv-word" style={{ ['--at' as string]: `${START + i * STEP}s` }}>
              {w}
            </span>{' '}
          </span>
        ))}
      </h1>
      <div className="mb-[4%]" />
    </div>
  );
}
