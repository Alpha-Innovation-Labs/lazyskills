import Link from 'next/link';
import { Github } from 'lucide-react';
import { Logo } from '@/components/logo';

export function Navbar() {
  return (
    <header className="border-b border-border/60 bg-background">
      <div className="mx-auto flex w-full max-w-[60rem] items-center justify-between px-6 py-3">
        <Link href="/" aria-label="Go to home" className="inline-flex items-center">
          <Logo className="h-5" />
        </Link>

        <div className="flex items-center gap-3">
          <Link
            href="/docs"
            className="text-sm font-medium text-muted-foreground transition hover:text-foreground"
          >
            Docs
          </Link>

          <a
            href="https://github.com/Alpha-Innovation-Labs/lazyskills"
            target="_blank"
            rel="noreferrer"
            aria-label="View repository on GitHub"
            className="text-muted-foreground transition hover:text-foreground"
          >
            <Github className="h-4 w-4" />
          </a>
        </div>
      </div>
    </header>
  );
}
