'use client';

import Link from 'next/link';
import { Check, Copy } from 'lucide-react';
import { useMemo, useState } from 'react';
import { OpenSkillsLogo } from '@/components/open-skills-logo';

const HOME_TEMPLATE = {
  installOptions: [
    {
      id: 'npm',
      label: 'npx',
      command: 'npx lazyskills',
    },
    {
      id: 'cargo',
      label: 'Cargo',
      command: 'cargo install lazyskills',
    },
    {
      id: 'brew',
      label: 'Homebrew',
      command: 'brew install alpha-innovation-labs/tap/lazyskills',
    },
    {
      id: 'scoop',
      label: 'Scoop',
      command: 'scoop bucket add alpha-innovation-labs https://github.com/Alpha-Innovation-Labs/scoop-bucket && scoop install lazyskills',
    },
  ],
};

export function HeroTemplate() {
  const [active, setActive] = useState(HOME_TEMPLATE.installOptions[0]?.id ?? 'npm');
  const [copied, setCopied] = useState(false);

  const activeInstall = useMemo(
    () =>
      HOME_TEMPLATE.installOptions.find((option) => option.id === active) ??
      HOME_TEMPLATE.installOptions[0],
    [active],
  );

  async function copyInstallCommand() {
    if (!activeInstall) return;
    await navigator.clipboard.writeText(activeInstall.command);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <section className="relative isolate overflow-hidden flex-1">
      <div className="absolute inset-0 -z-10 bg-[linear-gradient(to_right,var(--color-fd-border)_1px,transparent_1px),linear-gradient(to_bottom,var(--color-fd-border)_1px,transparent_1px)] bg-[size:28px_28px] opacity-25" />
      <div className="absolute inset-0 -z-10 bg-[radial-gradient(circle_at_25%_25%,color-mix(in_oklab,var(--color-fd-primary)_30%,transparent),transparent_45%),radial-gradient(circle_at_75%_20%,color-mix(in_oklab,var(--color-fd-accent)_22%,transparent),transparent_45%)]" />
      <div className="mx-auto w-full max-w-5xl px-6 pb-16 pt-8 sm:pb-24 sm:pt-12 lg:pb-28 lg:pt-14">
        <div className="space-y-6 rounded-2xl border border-fd-border/80 bg-fd-card/75 p-8 shadow-2xl shadow-black/10 sm:p-10">
          <div className="mb-9 sm:mb-12">
            <OpenSkillsLogo />
          </div>

          <div className="border border-fd-border/80 bg-black/95">
            <div className="flex flex-wrap border-b border-fd-border/80 bg-black">
              {HOME_TEMPLATE.installOptions.map((option) => {
                const activeClass =
                  option.id === active
                    ? 'bg-fd-primary/95 text-fd-primary-foreground'
                    : 'bg-black text-fd-foreground hover:bg-fd-accent/15';

                return (
                  <button
                    key={option.id}
                    type="button"
                    onClick={() => setActive(option.id)}
                    className={`cursor-pointer border-r border-fd-border/70 px-4 py-2 text-xs font-medium transition last:border-r-0 ${activeClass}`}
                  >
                    {option.label}
                  </button>
                );
              })}
            </div>

            <div className="flex items-center gap-3 bg-black px-4 py-3">
              <span className="text-fd-primary">$</span>
              <code className="flex-1 overflow-x-auto text-sm">{activeInstall?.command}</code>
              <button
                type="button"
                onClick={copyInstallCommand}
                className="inline-flex h-8 w-8 cursor-pointer items-center justify-center border border-fd-border/80 bg-black transition hover:bg-fd-accent/20"
                aria-label="Copy install command"
              >
                {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
              </button>
            </div>
          </div>

          <div className="mt-8 overflow-hidden rounded-xl border border-fd-border/80 bg-black/70 sm:mt-10">
            <img
              src="/media/lazyskills-demo.gif"
              alt="Lazyskills demo showing skill discovery in the terminal UI"
              className="w-full"
            />
          </div>
        </div>
      </div>
    </section>
  );
}
