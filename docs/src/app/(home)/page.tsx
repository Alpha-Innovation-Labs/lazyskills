import { Github } from 'lucide-react';
import Link from 'next/link';
import { InstallCommandBlock } from '@/components/install-command-block';
import { Button } from '@/components/ui/button';

const INSTALL_OPTIONS = [
  {
    id: 'npm',
    label: 'npx',
    command: 'npx lazyskills',
  },
  {
    id: 'cargo',
    label: 'cargo',
    command: 'cargo install lazyskills',
  },
  {
    id: 'brew',
    label: 'homebrew',
    command: 'brew install alpha-innovation-labs/tap/lazyskills',
  },
  {
    id: 'scoop',
    label: 'scoop',
    command:
      'scoop bucket add alpha-innovation-labs https://github.com/Alpha-Innovation-Labs/scoop-bucket && scoop install lazyskills',
    displayLines: [
      'scoop bucket add alpha-innovation-labs https://github.com/Alpha-Innovation-Labs/scoop-bucket',
      'scoop install lazyskills',
    ],
  },
];

export default function HomePage() {
  return (
    <section className="flex-1 bg-background">
      <div className="mx-auto w-full max-w-[60rem] px-5 pb-16 sm:px-6 sm:pb-24 lg:px-7 lg:pb-28">
        <div className="space-y-8 sm:space-y-10">
          <div className="space-y-5 py-12 text-center sm:py-16 md:py-20">
            <h1 className="text-4xl font-normal tracking-tight text-foreground sm:text-5xl">
              Open-source skill management for <span className="whitespace-nowrap">coding agents</span>
            </h1>

            <div className="flex flex-wrap items-center justify-center gap-3">
              <Button asChild>
                <Link href="/docs">Documentation</Link>
              </Button>

              <Button asChild variant="outline">
                <a
                  href="https://github.com/Alpha-Innovation-Labs/lazyskills"
                  target="_blank"
                  rel="noreferrer"
                >
                  <Github className="mr-2 h-4 w-4" />
                  View on GitHub
                </a>
              </Button>
            </div>
          </div>

          <InstallCommandBlock options={INSTALL_OPTIONS} />

          <p className="text-left text-sm leading-relaxed text-foreground/70 sm:text-[15px]">
            Lazyskills is an open-source skill management layer for coding agents. It gives you a
            fast terminal UI to discover, preview, and manage skills from one place. Under the hood,
            it uses the official <code className="font-mono text-sm">skills</code> CLI for
            install/remove operations, so behavior stays compatible with your existing setup. You get
            a cleaner workflow without replacing the tooling you already trust.
          </p>

          <div className="overflow-hidden">
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
