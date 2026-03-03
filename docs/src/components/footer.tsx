import { Github } from 'lucide-react';

export function Footer() {
  const currentYear = new Date().getFullYear();

  return (
    <footer className="mt-auto">
      <div className="mx-auto flex w-full max-w-[60rem] items-center justify-between border-t border-border/60 px-5 py-4 text-xs text-foreground/80 sm:px-6 lg:px-7">
        <p>&copy; {currentYear} Lazyskills</p>

        <a
          href="https://github.com/Alpha-Innovation-Labs/lazyskills"
          target="_blank"
          rel="noreferrer"
          aria-label="Lazyskills on GitHub"
          className="text-foreground/80 transition-colors hover:text-foreground"
        >
          <Github className="h-4 w-4" />
        </a>
      </div>
    </footer>
  );
}
