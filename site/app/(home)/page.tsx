import Link from 'next/link';
import { CaptionHero } from '@/components/caption-hero';
import { InstallTabs } from '@/components/install-tabs';

const STEPS = [
  { cmd: 'scriptvox plan --brief brief.json', what: 'Reads your script and clips. Shows the scenes, the time and the cost before anything is made.' },
  { cmd: 'scriptvox make --brief brief.json', what: 'Speaks the script, cuts the clips to the words, draws the captions and cards, and writes one MP4.' },
  { cmd: 'scriptvox check <run>', what: 'Checks the result: sound level, caption timing, text on screen, frozen frames. Each fault comes with its fix.' },
];

export default function HomePage() {
  return (
    <main className="mx-auto w-full max-w-[1200px] px-4 sm:px-8">
      <section className="pt-10 sm:pt-16">
        <CaptionHero />
        <div className="mt-10 grid gap-10 md:grid-cols-[minmax(0,1fr)_minmax(0,1.5fr)] md:items-start [&>*]:min-w-0">
          <p className="max-w-[46ch] text-lg leading-relaxed text-[var(--sv-chalk)]">
            ScriptVox is a command-line tool. Give it a script and your clips, and it makes a narrated, captioned MP4.
            You run it, or a coding agent runs it for you. One binary. No account. No telemetry.
          </p>
          <InstallTabs />
        </div>
      </section>

      <section className="mt-28 sm:mt-36">
        <h2 className="text-3xl sm:text-4xl font-bold [font-stretch:112%] tracking-[-0.01em]">Three commands</h2>
        <ol className="mt-10 grid gap-px bg-[var(--sv-rule)] border border-[var(--sv-rule)] md:grid-cols-3">
          {STEPS.map((s, i) => (
            <li key={s.cmd} className="bg-[var(--sv-ink)] p-6">
              <span className="font-mono text-xs text-[var(--sv-caption)]">{i + 1}</span>
              <code className="mt-3 block font-mono text-sm text-[var(--sv-chalk)]">{s.cmd}</code>
              <p className="mt-4 text-[var(--sv-smoke)] leading-relaxed">{s.what}</p>
            </li>
          ))}
        </ol>
        <p className="mt-6 max-w-[64ch] text-[var(--sv-smoke)] leading-relaxed">
          A plan that costs money stops and waits for your approval. A run that stops continues from its last finished
          chunk.
        </p>
      </section>

      <section className="mt-28 sm:mt-36 grid gap-10 md:grid-cols-2 [&>*]:min-w-0">
        <div>
          <h2 className="text-3xl sm:text-4xl font-bold [font-stretch:112%] tracking-[-0.01em]">Made for agents too</h2>
          <p className="mt-6 max-w-[52ch] text-[var(--sv-smoke)] leading-relaxed">
            Every command answers in JSON with the next step and the fix for each fault. Claude Code and other coding
            agents can plan, make and check a video without a person in the loop, and stop at the price.
          </p>
          <p className="mt-6">
            <Link href="/docs" className="underline decoration-[var(--sv-caption)] underline-offset-4 hover:text-[var(--sv-caption)]">
              Read the docs
            </Link>
          </p>
        </div>
        <pre className="m-0 overflow-x-auto border border-[var(--sv-rule)] p-5 font-mono text-[13px] leading-relaxed text-[var(--sv-smoke)]">
{`{
  "ok": false,
  "status": "approval_needed",
  "command": "make",
  "plan_id": "p_e0dbc84426203d19",
  "error": {
    "code": "SVX_APPROVAL_NEEDED",
    "message": "approval needed: this plan costs money ($0.42)",
    "fix": "Run the same command with --approve p_e0dbc84426203d19"
  },
  "metrics": { "cost_usd": 0.42 }
}`}
        </pre>
      </section>

      <footer className="mt-32 mb-12 flex flex-wrap gap-x-8 gap-y-3 border-t border-[var(--sv-rule)] pt-8 text-sm text-[var(--sv-smoke)]">
        <span>ScriptVox, Apache-2.0</span>
        <a className="hover:text-[var(--sv-chalk)]" href="https://github.com/scriptvox/scriptvox">GitHub</a>
        <a className="hover:text-[var(--sv-chalk)]" href="https://github.com/scriptvox/scriptvox/releases">Releases</a>
        <a className="hover:text-[var(--sv-chalk)]" href="https://huggingface.co/scriptvox">Hugging Face</a>
        <Link className="hover:text-[var(--sv-chalk)]" href="/llms.txt">llms.txt</Link>
      </footer>
    </main>
  );
}
