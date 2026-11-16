'use client';
import { useState } from 'react';

const ROUTES = [
  { name: 'Linux and macOS', cmd: 'curl -fsSL https://github.com/scriptvox/scriptvox/releases/latest/download/install.sh | sh' },
  { name: 'Homebrew', cmd: 'brew install scriptvox/tap/scriptvox' },
  { name: 'Scoop', cmd: 'scoop bucket add scriptvox https://github.com/scriptvox/scriptvox\nscoop install scriptvox' },
  { name: 'Ubuntu and Debian', cmd: 'sudo apt install ./scriptvox_0.1.0_amd64.deb' },
  { name: 'Windows', cmd: 'irm https://github.com/scriptvox/scriptvox/releases/latest/download/install.ps1 -OutFile install.ps1\npowershell -ExecutionPolicy Bypass -File install.ps1' },
];

export function InstallTabs() {
  const [i, setI] = useState(0);
  const [copied, setCopied] = useState(false);
  const r = ROUTES[i];
  return (
    <div className="border border-[var(--sv-rule)]">
      <div role="tablist" aria-label="Install route" className="flex overflow-x-auto border-b border-[var(--sv-rule)]">
        {ROUTES.map((x, n) => (
          <button
            key={x.name}
            role="tab"
            aria-selected={n === i}
            onClick={() => {
              setI(n);
              setCopied(false);
            }}
            className={`shrink-0 px-4 py-2.5 text-sm transition-colors ${
              n === i ? 'text-[var(--sv-ink)] bg-[var(--sv-caption)]' : 'text-[var(--sv-smoke)] hover:text-[var(--sv-chalk)]'
            }`}
          >
            {x.name}
          </button>
        ))}
      </div>
      <div role="tabpanel" className="relative">
        <pre className="m-0 overflow-x-auto p-5 pr-24 font-mono text-sm leading-relaxed text-[var(--sv-chalk)]">
          {r.cmd.split('\n').map((l) => (
            <div key={l}>
              <span className="select-none text-[var(--sv-smoke)]">$ </span>
              {l}
            </div>
          ))}
        </pre>
        <button
          type="button"
          onClick={() => navigator.clipboard.writeText(r.cmd).then(() => setCopied(true))}
          className="absolute right-3 top-3 px-3 py-1.5 text-xs bg-[var(--sv-ink)] border border-[var(--sv-rule)] text-[var(--sv-smoke)] hover:text-[var(--sv-chalk)] hover:border-[var(--sv-smoke)]"
        >
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
    </div>
  );
}
